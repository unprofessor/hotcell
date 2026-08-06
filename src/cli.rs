//! Developer-facing command-line interface.
//!
//! Mirrors the `DeveloperCLI` surface: status discovery is per-directory
//! (scoped to `$PWD/Cellfile`), and the developer can run and destroy cells.
//! `hotcell run` blocks until the program exits, relaying stdio faithfully
//! and passing the exit code through.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::Stdio;
use std::thread;

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};

use crate::cell::CellStatus;
use crate::cellfile::Cellfile;
use crate::isolation::AgentBridge;
use crate::provisioning::{provision, provision_digest, ProvisionOutcome};
use crate::state;

#[derive(Parser)]
#[command(name = "hotcell", version, about = "An AI coding agent sandbox")]
struct Cli {
    /// Path to the Cellfile, overriding the conventional `$PWD/Cellfile`.
    /// Applies to every subcommand. The cell's state directory lives
    /// alongside the given file unless `--cell` overrides it.
    #[arg(short = 'f', long = "file", global = true, value_name = "CELLFILE")]
    file: Option<String>,
    /// Cell directory, overriding the Cellfile's parent as the root for cell
    /// state (`.cell/...`). Applies to every subcommand. Use `--file` to read
    /// a Cellfile from one place while keeping state in another.
    #[arg(short = 'c', long = "cell", global = true, value_name = "DIR")]
    cell: Option<String>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run a program inside a cell, provisioning first if needed.
    Run {
        /// Cell name. Defaults to "default".
        #[arg(long, default_value = state::DEFAULT_CELL_NAME)]
        name: String,
        /// Program to execute inside the cell.
        program: String,
        /// Arguments to pass to the program.
        args: Vec<String>,
    },
    /// Destroy a cell's provisioned filesystem, returning it to unprovisioned.
    Destroy {
        /// Cell name. Defaults to "default".
        #[arg(long, default_value = state::DEFAULT_CELL_NAME)]
        name: String,
    },
    /// List cells for the Cellfile.
    Status,
    /// INTERNAL: in-namespace loopback-to-Unix-socket forwarder supervisor.
    ///
    /// Not for developers. Launched by `build_agent_command` inside the
    /// agent's `bwrap --unshare-net` namespace when the cell has a non-empty
    /// network policy. Binds the namespace's own `127.0.0.1`, relays each
    /// accepted TCP connection byte-for-byte to the bridge Unix socket
    /// (`--uds`), sets `HTTP_PROXY`/`HTTPS_PROXY`/`NO_PROXY` for the agent to
    /// point at this loopback address, then spawns and waits on the agent
    /// program. Exits with the agent's exit code.
    #[command(hide = true)]
    Fwd {
        /// In-sandbox path of the bridge Unix socket to relay to.
        #[arg(long)]
        uds: String,
        /// The agent program to supervise.
        program: String,
        /// Arguments for the agent program. Captured verbatim (trailing) so
        /// flags after the program are passed through untouched.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let ctx = ResolveCtx {
        file: cli.file.as_deref(),
        cell: cli.cell.as_deref(),
    };
    match cli.command {
        Command::Run {
            name,
            program,
            args,
        } => run_cell(&ctx, &name, &program, &args),
        Command::Destroy { name } => destroy_cell(&ctx, &name),
        Command::Status => status(&ctx),
        Command::Fwd { uds, program, args } => run_fwd(&uds, &program, &args),
    }
}

/// Root-level context: where to read the Cellfile from and where to root
/// cell state. Both default to the Cellfile's parent (or `$PWD`).
struct ResolveCtx<'a> {
    file: Option<&'a str>,
    cell: Option<&'a str>,
}

/// Resolve the Cellfile and its cell directory from the root-level context.
/// `--file` selects the Cellfile path; `--cell` overrides the cell directory
/// (the root for state) independently. By default both derive from
/// `$PWD/Cellfile`.
fn resolve(ctx: &ResolveCtx) -> Result<Cellfile> {
    let mut cellfile = match ctx.file {
        Some(p) => Cellfile::read_at(std::path::Path::new(p))?,
        None => {
            let cwd = std::env::current_dir()?;
            Cellfile::read(&cwd)?
        }
    };
    if let Some(dir) = ctx.cell {
        cellfile.directory = PathBuf::from(dir);
    }
    Ok(cellfile)
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn run_cell(ctx: &ResolveCtx, name: &str, program: &str, args: &[String]) -> Result<()> {
    let cellfile = resolve(ctx)?;

    let declaration = cellfile.declaration.clone();

    let mut cell = state::load_cell(&cellfile, name)?;

    // Carry the program/args through the provisioning phase in memory.
    cell.pending_program = Some(program.to_string());
    cell.pending_args = args.to_vec();

    // Skip re-provisioning when the cell is already provisioned with the
    // current declaration AND the provisioning inputs are unchanged. The
    // shell provisioner records a SHA-256 of its script in
    // `provisioned_as.json`; if that digest still matches the script on
    // disk, re-running the provisioner would only reproduce the same rootfs,
    // so we short-circuit. A mismatch (or a legacy record with no digest)
    // falls through to a full re-provision.
    let current_digest = provision_digest(&cellfile.directory, &declaration.provisioner)?;
    let already_current =
        cell.is_current(&declaration) && cell.provisioned_script_digest == current_digest;
    if already_current {
        eprintln!(
            "cell {:?} already provisioned with current inputs; skipping provisioning",
            name
        );
    } else {
        // Enter the provisioning phase: clear durable markers so a crash leaves
        // the cell unprovisioned rather than stuck.
        state::begin_provisioning(&cellfile.directory, name)?;

        // Provisioning phase: the provisioner seeds the
        // cell's rootfs. Mirrors ProvisionSucceeds / ProvisionFails and the
        // ProvisioningLogsToConsole guarantee.
        let outcome = provision(&cell, &declaration)?;
        let declaration = match outcome {
            ProvisionOutcome::Succeeded(d) => d,
            ProvisionOutcome::Failed(err) => {
                state::mark_failed(&cellfile.directory, name)?;
                eprintln!("provisioning failed: {err}");
                std::process::exit(1);
            }
        };
        state::mark_provisioned(
            &cellfile.directory,
            name,
            &declaration,
            current_digest.as_deref(),
        )?;
        cell.provisioned_as = Some(declaration);
        cell.provisioned_script_digest = current_digest;
    }

    // Launch the agent inside the isolated sandbox, relaying stdio.
    // Mirrors RelayOnly / CleanEnvironment / ProvisionedEnvironment.
    let provisioned = cell.provisioned_as.as_ref().expect("just provisioned");
    let env: Vec<(String, String)> = provisioned
        .environment
        .iter()
        .map(|v| (v.key.clone(), v.value.clone()))
        .collect();

    // In-sandbox workdir from the provisioned declaration; config default is
    // "/work". The host-side backing lives under the cell's rootfs and is
    // created by the provisioner (or the safety-net below).
    let workdir = if provisioned.workdir.is_empty() {
        "/work".to_string()
    } else {
        provisioned.workdir.clone()
    };

    let cell_fs = state::fs_dir(&cellfile.directory, name);
    // Safety net: ensure the workdir exists even if a provisioner forgot to.
    let workdir_host = crate::provisioning::workdir_host_path(&cell_fs, &workdir);
    std::fs::create_dir_all(&workdir_host)?;

    // Network firewall (HTTP allowlist proxy) with a loopback-only bridge.
    //
    // Empty policy (offline): no firewall, no bridge — the agent stays fully
    // offline via `--unshare-net` (added by `build_agent_command`). This is
    // the airtight offline path and must not change.
    //
    // Non-empty policy: start the firewall proxy on the host loopback, then
    // bridge it into the agent's `--unshare-net` namespace via a Unix-domain
    // socket (host side: `FirewallHandle::start_uds_bridge`; namespace side:
    // the `hotcell fwd` supervisor started by `build_agent_command`). The
    // agent's HTTP_PROXY points at the in-namespace forwarder's loopback
    // address — set by the forwarder itself once it discovers its bound port —
    // so from the agent's view the proxy is on its own loopback and nothing
    // else is reachable. `HTTP_PROXY`/`HTTPS_PROXY`/`NO_PROXY` are therefore
    // NOT set here; the forwarder sets them on the agent child.
    //
    // The bridge dir is cell-scoped (under the cell rootfs) so no other cell
    // or host process can squat it, and is bind-mounted read-only into the
    // sandbox so the agent cannot tamper with the socket. It is removed after
    // the agent exits.
    let bridge_dir_host: Option<(AgentBridge, PathBuf, crate::firewall::FirewallHandle)> =
        if !provisioned.network.allowed_endpoints.is_empty() {
            let handle = crate::firewall::start(&provisioned.network)?;
            eprintln!(
                "network firewall: HTTP allowlist proxy on {} ({} endpoint(s))",
                handle.listen_addr(),
                provisioned.network.allowed_endpoints.len()
            );
            // Cell-scoped bridge directory under the cell rootfs. Inside the
            // sandbox this appears at /hotcell-bridge; the host writes the
            // Unix socket here and the in-namespace forwarder connects to it.
            let bridge_dir = cell_fs.join("hotcell-bridge");
            std::fs::create_dir_all(&bridge_dir)?;
            let uds_host_path = bridge_dir.join("proxy.sock");
            handle.start_uds_bridge(&uds_host_path)?;
            let bridge = AgentBridge {
                host_bridge_dir: bridge_dir.clone(),
                uds_in_sandbox: "/hotcell-bridge/proxy.sock".to_string(),
            };
            // Hold the firewall handle for the lifetime of the agent so the
            // proxy (and its UDS bridge) stay up. Bound below to keep the
            // borrow alive across the spawn.
            Some((bridge, bridge_dir, handle))
        } else {
            None
        };

    // Split the bridge triple into the config (borrowed by build_agent_command)
    // and the guards (handle + dir path) that must outlive the child.
    let (bridge_cfg, bridge_guard_dir, _firewall): (
        Option<AgentBridge>,
        Option<PathBuf>,
        Option<crate::firewall::FirewallHandle>,
    ) = match bridge_dir_host {
        Some((b, dir, h)) => (Some(b), Some(dir), Some(h)),
        None => (None, None, None),
    };

    let log_file = state::log_file_path(&cellfile.directory, name);

    let mut cmd = crate::isolation::build_agent_command(
        &cell_fs,
        &workdir,
        &env,
        bridge_cfg.as_ref(),
        program,
        args,
    );
    cmd.stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    eprintln!("session log: {}", log_file.display());

    let mut child = cmd.spawn()?;
    let exit_status = child.wait()?;

    // The agent has exited; tear down the firewall proxy and the bridge dir
    // (if any) before we return. Explicit drops because the function exits
    // via `process::exit`, which would otherwise skip `Drop`.
    drop(_firewall);
    if let Some(dir) = bridge_guard_dir {
        // Best-effort cleanup of the cell-scoped bridge dir + socket. A
        // failure leaves a harmless empty dir / stale socket under the cell
        // rootfs, cleaned on the next run (start_uds_bridge unlinks a stale
        // socket before binding).
        let _ = std::fs::remove_dir_all(&dir);
    }

    if !exit_status.success() {
        eprintln!(
            "session completed with errors: exit code {}",
            exit_status.code().unwrap_or(-1)
        );
    }

    let code = exit_status.code().unwrap_or(-1);
    std::process::exit(code);
}

fn destroy_cell(ctx: &ResolveCtx, name: &str) -> Result<()> {
    let cellfile = resolve(ctx)?;
    let cell = state::load_cell(&cellfile, name)?;
    // Mirrors DestroyCell: only provisioned or failed cells can be destroyed.
    if !matches!(
        cell.status(),
        CellStatus::Provisioned | CellStatus::ProvisioningFailed
    ) {
        bail!(
            "cell {:?} is not in a destroyable state ({})",
            name,
            cell.status().as_str()
        );
    }
    state::destroy_cell(&cellfile.directory, name)?;
    println!("destroyed cell {:?}", name);
    Ok(())
}

fn status(ctx: &ResolveCtx) -> Result<()> {
    let cellfile = resolve(ctx)?;
    let names = state::list_cell_names(&cellfile.directory)?;
    if names.is_empty() {
        println!("no cells in {}", cellfile.directory.display());
        return Ok(());
    }
    for name in names {
        let cell = state::load_cell(&cellfile, &name)?;
        let current = cell.is_current(&cellfile.declaration);
        println!(
            "{:<16} {:<20} {}",
            cell.name,
            cell.status().as_str(),
            if current { "current" } else { "stale" }
        );
    }
    Ok(())
}

/// The in-namespace forwarder supervisor (`hotcell fwd`).
///
/// Runs *inside* the agent's `bwrap --unshare-net` namespace alongside the
/// agent. Binds the namespace's own `127.0.0.1` on an OS-assigned port and
/// relays each accepted TCP connection byte-for-byte to the bridge Unix
/// socket (`uds_path`) — which the host-side firewall UDS bridge connects to
/// the firewall proxy. The agent's `HTTP_PROXY`/`HTTPS_PROXY` are set to this
/// loopback address so the agent sees the proxy on its own loopback; nothing
/// else is reachable (the namespace has only loopback; non-loopback egress is
/// `ENETUNREACH`).
///
/// This is a deliberately tiny, stdlib-only TCP→Unix-socket relay. It holds
/// no policy logic: enforcement stays in the firewall proxy. It uses blocking
/// threads (two per connection, one per direction) with half-close on EOF so a
/// short write on one side does not deadlock the other. When the agent child
/// exits, the supervisor exits with its code, which tears down the listener
/// and all relay threads.
fn run_fwd(uds_path: &str, program: &str, args: &[String]) -> Result<()> {
    // Bind the namespace's loopback. Port 0 lets the OS pick a free port in
    // this (isolated) namespace; we learn the real port and feed it to the
    // agent as HTTP_PROXY. This avoids a fixed port that could collide with
    // an agent service.
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    let proxy_url = format!("http://127.0.0.1:{port}");

    // Acceptor thread: for each inbound TCP connection from the agent, spawn a
    // relay that opens the bridge Unix socket and copies bytes both ways.
    let uds = uds_path.to_string();
    let _acceptor = thread::spawn(move || {
        // accept errors mean the listener is closing (we are exiting); stop.
        for stream in listener.incoming() {
            match stream {
                Ok(tcp) => {
                    let uds = uds.clone();
                    thread::spawn(move || {
                        let _ = relay_tcp_to_uds(tcp, &uds);
                    });
                }
                Err(_) => break,
            }
        }
    });

    // Spawn the agent. It inherits this process's environment (the declared
    // cell env, set by bwrap --clearenv + --setenv) plus the proxy vars we
    // add here. We do NOT set NO_PROXY for any external host — loopback stays
    // direct so the agent can reach the forwarder itself.
    let mut child = std::process::Command::new(program);
    child.args(args);
    child.env("HTTP_PROXY", &proxy_url);
    child.env("HTTPS_PROXY", &proxy_url);
    // Loopback stays direct: the forwarder is on 127.0.0.1, and any local
    // services the agent runs should not be forced through the proxy.
    child.env("NO_PROXY", "127.0.0.1,localhost");
    child
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    let mut child = child.spawn()?;
    let status = child.wait()?;

    // Dropping the listener would be ideal, but it is owned by the acceptor
    // thread. Exiting the process closes the socket and reaps the threads.
    let code = status.code().unwrap_or(-1);
    std::process::exit(code);
}

/// Relay one agent TCP connection to the bridge Unix socket, byte-for-byte in
/// both directions until either side closes. Two threads (one per direction);
/// when a direction hits EOF it shuts down the peer's write half so the other
/// thread unblocks instead of hanging on a half-open connection.
fn relay_tcp_to_uds(tcp: std::net::TcpStream, uds_path: &str) -> std::io::Result<()> {
    let uds = UnixStream::connect(uds_path)?;
    let tcp_a = tcp.try_clone()?;
    let uds_a = uds.try_clone()?;
    // tcp -> uds
    let t1 = thread::spawn(move || pipe(tcp_a, uds_a));
    // uds -> tcp
    let t2 = thread::spawn(move || pipe(uds, tcp));
    let _ = t1.join();
    let _ = t2.join();
    Ok(())
}

/// Copy bytes from `r` to `w` until EOF or error, then shut down `w` for
/// writes so the other direction of the relay unblocks (half-close instead of
/// a full close, which would also kill the still-live direction).
fn pipe<R: Read, W: Write + HalfClose>(mut r: R, mut w: W) {
    let mut buf = [0u8; 8192];
    loop {
        match r.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if w.write_all(&buf[..n]).is_err() {
                    break;
                }
            }
        }
    }
    let _ = w.shutdown_write();
}

/// Shutdown the write half of a stream. Implemented for the two concrete
/// stream types the relay uses so `pipe` can half-close generically.
trait HalfClose {
    fn shutdown_write(&self) -> std::io::Result<()>;
}

impl HalfClose for std::net::TcpStream {
    fn shutdown_write(&self) -> std::io::Result<()> {
        self.shutdown(std::net::Shutdown::Write)
    }
}

impl HalfClose for UnixStream {
    fn shutdown_write(&self) -> std::io::Result<()> {
        self.shutdown(std::net::Shutdown::Write)
    }
}
