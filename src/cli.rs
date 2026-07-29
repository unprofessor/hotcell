//! Developer-facing command-line interface.
//!
//! Mirrors the `DeveloperCLI` surface: status discovery is per-directory
//! (scoped to `$PWD/Cellfile`), and the developer can run and destroy cells.
//! `hotcell run` blocks until the program exits, relaying stdio faithfully
//! and passing the exit code through.

use std::path::PathBuf;
use std::process::Stdio;

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};

use crate::cell::CellStatus;
use crate::cellfile::Cellfile;
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
    let mut env: Vec<(String, String)> = provisioned
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

    // Network firewall (HTTP allowlist proxy): when the provisioned policy
    // declares allowed endpoints, start the local proxy and point the agent
    // at it via HTTP_PROXY/HTTPS_PROXY. Loopback stays direct via NO_PROXY so
    // the agent can still talk to itself. The firewall handle owns the proxy's
    // tokio runtime; hold it for the lifetime of the agent process so the
    // server stays up, and drop it once the child exits. A policy with no
    // allowed endpoints starts no firewall: the agent stays fully offline via
    // `--unshare-net` (added by `build_agent_command`).
    let _firewall: Option<crate::firewall::FirewallHandle> =
        if !provisioned.network.allowed_endpoints.is_empty() {
            let handle = crate::firewall::start(&provisioned.network)?;
            let proxy_url = format!("http://{}", handle.listen_addr());
            eprintln!(
                "network firewall: HTTP allowlist proxy on {} ({} endpoint(s))",
                handle.listen_addr(),
                provisioned.network.allowed_endpoints.len()
            );
            env.push(("HTTP_PROXY".to_string(), proxy_url.clone()));
            env.push(("HTTPS_PROXY".to_string(), proxy_url));
            // Keep loopback direct: the proxy only forwards CONNECT for
            // listed endpoints, and local services should not be forced
            // through it.
            env.push(("NO_PROXY".to_string(), "127.0.0.1,localhost".to_string()));
            Some(handle)
        } else {
            None
        };

    let log_file = state::log_file_path(&cellfile.directory, name);

    let mut cmd = crate::isolation::build_agent_command(&cell_fs, &workdir, &env, program, args);
    cmd.stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    eprintln!("session log: {}", log_file.display());

    let mut child = cmd.spawn()?;
    let exit_status = child.wait()?;

    // The agent has exited; tear down the firewall proxy (if any) before we
    // return. Explicit drop because the function exits via `process::exit`,
    // which would otherwise skip the handle's `Drop`.
    drop(_firewall);

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
