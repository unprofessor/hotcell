//! Filesystem isolation and the two sandbox permission profiles.
//!
//! Both the provisioner and the agent run inside bubblewrap (`bwrap`) with
//! the cell's provisioned rootfs bind-mounted as `/`. They differ in *risk
//! profile* — the permissions model that separates a trusted, developer-
//! authored provisioner from an untrusted, LLM-directed agent:
//!
//! |                       | Provisioning profile | Agent profile |
//! |-----------------------|----------------------|---------------|
//! | Cell rootfs (`/`)     | read-write           | read-write    |
//! | Base system dirs      | read-only            | read-only     |
//! | Declared host paths   | read-only, staged    | **none**      |
//! | Cellfile directory    | read-only, staged    | **none**      |
//! | Network               | shared (egress)      | offline/firewall |
//! | Environment           | host env + controls  | clean (declared only) |
//!
//! The provisioner may bootstrap (read host tools, fetch packages); the agent
//! may not. Neither can write to the host — all host binds are read-only.
//! Mirrors the `ProvisionerRunsInCell`, `BootstrapRiskProfile`,
//! `ProvisionerCannotWriteHost`, `ProvisionerCleanEnvironment`,
//! `FilesystemIsolation`, and `ProvisionedEnvironment` guarantees.
//!
//! ## Host-path staging
//!
//! Host paths (the Cellfile directory and `host-path` entries) are
//! bind-mounted under a neutral staging prefix ([`HOST_STAGING_DIR`], `/hotcell`)
//! rather than at their original absolute host paths. This is critical: bwrap
//! creates intermediate directories in the rootfs for bind targets whose paths
//! do not already exist, and those stubs persist after the sandbox exits. If
//! host paths were bound at their original locations (e.g. `/home/user/.pi`),
//! the rootfs would inherit a `/home/user/...` skeleton that leaks the host
//! username and directory structure to the agent. Staging under `/hotcell`
//! confines all such stubs to a single directory that the caller removes from
//! the rootfs after provisioning completes.

use std::path::{Component, Path, PathBuf};
use std::process::Command;

/// One bubblewrap argument: a flag plus the values it consumes.
///
/// The builders compose commands as `Argument`s so the structure is explicit
/// (a flag and its values) rather than a flat argv. The execution plan
/// renders one line per argument, and [`collapse`] flattens the list into the
/// `Command` that actually runs — so the rendered `--flag v1 v2` and the
/// executed argv are the same thing, symbolically and concretely.
///
/// The flag may also be `--`, the separator between bwrap's options and the
/// command it runs inside the sandbox; its values are that command and its
/// arguments.
#[derive(Debug, Clone)]
pub struct Argument {
    /// The flag, e.g. `--bind`, `--setenv`, or `--` (the command separator).
    pub flag: String,
    /// Values following the flag, e.g. the source and target of a bind.
    pub values: Vec<String>,
}

impl Argument {
    /// Compose a flag and the values it consumes.
    pub fn new(flag: &str, values: impl IntoIterator<Item = String>) -> Argument {
        Argument {
            flag: flag.to_string(),
            values: values.into_iter().collect(),
        }
    }
}

/// Collapse planned arguments into an executable `Command`, preserving argv
/// order (flag first, then its values, then the next flag). This is the only
/// place a `bwrap` `Command` is constructed from an argument list.
pub fn collapse(arguments: &[Argument]) -> Command {
    let mut cmd = Command::new("bwrap");
    for arg in arguments {
        cmd.arg(&arg.flag);
        for value in &arg.values {
            cmd.arg(value);
        }
    }
    cmd
}

/// In-sandbox mount point for the bind-mounted hotcell binary, used as the
/// forwarder supervisor when a network bridge is active. Lives outside the
/// cell rootfs proper so the agent cannot see or tamper with the supervisor
/// binary (the bind is read-only).
const FORWARDER_BIN: &str = "/hotcell-fwd";

/// In-sandbox mount point for the shared Unix-socket bridge directory.
/// Bind-mounted read-only from a cell-scoped host directory so the agent can
/// neither write to nor replace the bridge socket (it can only be connected
/// to by the in-namespace forwarder, which is hotcell-owned code).
pub const BRIDGE_DIR: &str = "/hotcell-bridge";

/// Base-system directories layered read-only on top of the cell rootfs in
/// both profiles. Provisioners should install tools under paths these do not
/// shadow (e.g. `/opt`, `/work`, `/home`).
const BASE_RO_BINDS: &[(&str, &str)] = &[
    ("/usr", "/usr"),
    ("/lib", "/lib"),
    ("/lib64", "/lib64"),
    ("/bin", "/bin"),
    ("/etc", "/etc"),
];

/// Neutral staging prefix inside the provisioning sandbox. The Cellfile
/// directory is mounted at `{prefix}/cellfile` and each declared host path at
/// `{prefix}/host/<original>`. The caller removes `{cell_root}/{prefix}` from
/// the rootfs after provisioning so no host-path stubs leak to the agent.
pub const HOST_STAGING_DIR: &str = "/hotcell";

/// Expand a leading `~` to the caller's `HOME`. bwrap requires absolute paths.
pub fn expand_home(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    if path == "~" {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home);
        }
    }
    PathBuf::from(path)
}

/// Make a host-side path absolute against hotcell's current working
/// directory, without touching the filesystem.
///
/// bwrap resolves relative bind sources against its own cwd at exec time —
/// the same cwd hotcell runs in — so the absolute form is behaviourally
/// identical, while the displayed plan is unambiguous and the copy-pasted
/// command works from any directory. `~`-expansion is handled separately by
/// [`expand_home`]. `.` components from joins are dropped lexically; `..`
/// is preserved, since resolving it correctly would need the filesystem.
pub fn absolutize(path: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }
    let Ok(cwd) = std::env::current_dir() else {
        return path.to_path_buf();
    };
    let mut abs = cwd;
    for component in path.components() {
        match component {
            Component::CurDir => {}
            other => abs.push(other.as_os_str()),
        }
    }
    abs
}

/// Build the **provisioning** profile command: the provisioner runs inside
/// the cell rootfs (`cell_fs` as `/`), with read-only access to the Cellfile
/// directory and the declared host bootstrap paths — all staged under
/// [`HOST_STAGING_DIR`] so no host-path stubs are left in the rootfs. Network
/// is shared; the environment is the host's plus the given control/env vars.
///
/// `script_rel` is the provision script path relative to the Cellfile
/// directory (e.g. `./provision.sh`); it is executed as
/// `{HOST_STAGING_DIR}/cellfile/<script_rel>` inside the sandbox.
///
/// The sandbox sets these staging variables (in addition to `control_env`):
/// - `HOTCELL_CELLFILE_DIR`: `{prefix}/cellfile` — the in-sandbox Cellfile dir.
/// - `HOTCELL_HOST_ROOT`: `{prefix}/host` — root of the staged host-path tree.
/// - `HOTCELL_HOST_HOME`: `{prefix}/host/<host HOME>` — the staged host home,
///   for scripts that copy config from `~/.pi` etc. (only meaningful when the
///   host HOME is a declared or implied bootstrap path).
///
/// The returned list ends with `Argument { flag: "--", values: [script] }`:
/// the `--` separator makes bwrap treat the script as its command, not as
/// another option.
pub fn build_provisioning_command(
    cell_fs: &Path,
    cellfile_directory: &Path,
    host_paths: &[String],
    control_env: &[(String, String)],
    script_rel: &str,
) -> Vec<Argument> {
    let cell_fs_str = absolutize(cell_fs)
        .to_str()
        .expect("cell rootfs path must be valid UTF-8")
        .to_string();
    let cellfile_dir_str = absolutize(cellfile_directory)
        .to_str()
        .expect("cellfile directory must be valid UTF-8")
        .to_string();

    let staging = HOST_STAGING_DIR;
    let cellfile_mount = format!("{staging}/cellfile");
    let host_root = format!("{staging}/host");
    let script_in_sandbox = {
        let rel = script_rel.trim_start_matches("./");
        format!("{cellfile_mount}/{rel}")
    };

    let mut arguments: Vec<Argument> = Vec::new();

    // Cell rootfs as the sandbox root, read-write.
    arguments.push(Argument::new("--bind", [cell_fs_str, "/".to_string()]));
    // Read-only base system.
    for (src, dst) in BASE_RO_BINDS {
        arguments.push(Argument::new(
            "--ro-bind",
            [src.to_string(), dst.to_string()],
        ));
    }
    // Fresh /tmp, /proc, /dev — before the staged binds below so that a
    // Cellfile/host path under /tmp is not masked (bwrap applies ops in order).
    arguments.push(Argument::new("--tmpfs", ["/tmp".to_string()]));
    arguments.push(Argument::new("--proc", ["/proc".to_string()]));
    arguments.push(Argument::new("--dev", ["/dev".to_string()]));

    // Stage the Cellfile directory read-only under the staging prefix so the
    // provision script and project-relative seeds resolve without leaving a
    // host-path skeleton in the rootfs.
    arguments.push(Argument::new(
        "--ro-bind",
        [cellfile_dir_str, cellfile_mount.clone()],
    ));

    // Stage each declared host bootstrap path read-only under
    // {staging}/host/<original absolute path>. These are the provisioner's
    // privilege; the agent never sees them.
    for hp in host_paths {
        let resolved = absolutize(&expand_home(hp));
        if let Some(s) = resolved.to_str() {
            let target = format!("{host_root}{s}");
            arguments.push(Argument::new("--ro-bind", [s.to_string(), target]));
        }
    }

    arguments.push(Argument::new("--unshare-pid", []));
    arguments.push(Argument::new("--share-net", []));
    arguments.push(Argument::new("--die-with-parent", []));
    arguments.push(Argument::new("--chdir", [cellfile_mount.clone()]));

    // Staging variables (encapsulate the staging scheme here).
    let host_home = std::env::var("HOME").unwrap_or_default();
    let host_home_staged = format!("{host_root}{host_home}");
    arguments.push(Argument::new(
        "--setenv",
        ["HOTCELL_CELLFILE_DIR".to_string(), cellfile_mount],
    ));
    arguments.push(Argument::new(
        "--setenv",
        ["HOTCELL_HOST_ROOT".to_string(), host_root],
    ));
    arguments.push(Argument::new(
        "--setenv",
        ["HOTCELL_HOST_HOME".to_string(), host_home_staged],
    ));

    // Caller-supplied control/env vars (HOTCELL_*, HOME override, etc.).
    for (k, v) in control_env {
        arguments.push(Argument::new("--setenv", [k.clone(), v.clone()]));
    }

    // The command bwrap runs inside the sandbox: the provision script.
    arguments.push(Argument::new("--", [script_in_sandbox]));
    arguments
}

/// Remove the host-path staging directory from the cell rootfs after
/// provisioning, so no host-path stubs leak into the agent's view. Safe to
/// call when the staging dir does not exist (e.g. the no-op provisioner).
pub fn clean_staging(cell_fs: &Path) {
    let staging = cell_fs.join(HOST_STAGING_DIR.trim_start_matches('/'));
    if staging.exists() {
        // Best-effort: a failure here leaves harmless empty stubs under
        // /hotcell, which the agent can see but which reveal no host state
        // beyond the staging prefix itself.
        let _ = std::fs::remove_dir_all(&staging);
    }
}

/// Configuration for the loopback-only network bridge used when a cell has
/// a non-empty network policy.
///
/// The agent runs in a `bwrap --unshare-net` namespace, so the kernel gives
/// it *only* a private loopback interface — non-loopback egress is
/// `ENETUNREACH` with no route to tamper with. To let the agent reach the
/// host-side firewall proxy without exposing any network route, a tiny
/// in-namespace forwarder (the hotcell binary itself, re-invoked as
/// `hotcell fwd`) listens on the namespace's `127.0.0.1` and relays bytes to
/// a Unix-domain socket. That socket lives in [`host_bridge_dir`] on the host
/// (bind-mounted read-only into the sandbox at [`BRIDGE_DIR`]) and is
/// connected on the host side to the firewall's TCP proxy by
/// [`crate::firewall::FirewallHandle::start_uds_bridge`].
///
/// `uds_in_sandbox` is the path the forwarder connects to (e.g.
/// `/hotcell-bridge/proxy.sock`); `host_bridge_dir` is the host-side source
/// for the read-only bind of [`BRIDGE_DIR`].
#[derive(Debug, Clone)]
pub struct AgentBridge {
    /// Host-side directory containing the bridge Unix socket, bind-mounted
    /// read-only into the sandbox at [`BRIDGE_DIR`]. Cell-scoped (under the
    /// cell rootfs) so no other cell or host process can squat it.
    pub host_bridge_dir: PathBuf,
    /// In-sandbox path of the bridge Unix socket, e.g.
    /// `/hotcell-bridge/proxy.sock`.
    pub uds_in_sandbox: String,
}
/// Build the **agent** profile command: the agent runs inside the cell rootfs
/// (`cell_fs` as `/`) with *no* host path access, a clean environment
/// containing only the declared `env` vars, and network access governed by
/// `bridge`.
///
/// Network handling:
/// - `bridge = None` (empty/offline policy): `--unshare-net` is kept, so the
///   agent has no network at all — a private loopback interface only, no
///   route to the host. This is the airtight offline path.
/// - `bridge = Some(_)`: `--unshare-net` is *still* kept (the namespace has
///   only loopback, so non-loopback egress is kernel-enforced
///   `ENETUNREACH`), and an in-namespace forwarder supervisor is launched
///   instead of the agent directly. The supervisor (the hotcell binary,
///   re-invoked as `hotcell fwd` and bind-mounted read-only into the sandbox)
///   listens on the namespace's `127.0.0.1`, relays bytes to the bridge
///   Unix socket, and spawns the agent with `HTTP_PROXY`/`HTTPS_PROXY`
///   pointing at its own loopback address. See [`AgentBridge`].
///
/// The agent itself never gets a network route: it talks only to its own
/// loopback, where the hotcell-owned forwarder enforces the indirection to
/// the host firewall proxy.
///
/// The returned list always ends with `Argument { flag: "--", values:
/// [command, args...] }` — the `--` separator shuts off bwrap's option
/// parsing. With a bridge, the command is the forwarder supervisor
/// (`/hotcell-fwd fwd --uds ... -- program args`); without one it is the
/// agent program itself.
pub fn build_agent_command(
    cell_fs: &Path,
    workdir: &str,
    env: &[(String, String)],
    bridge: Option<&AgentBridge>,
    program: &str,
    args: &[String],
) -> Vec<Argument> {
    let cell_fs_str = absolutize(cell_fs)
        .to_str()
        .expect("cell rootfs path must be valid UTF-8")
        .to_string();

    let mut arguments: Vec<Argument> = Vec::new();

    // Cell rootfs as the sandbox root.
    arguments.push(Argument::new("--bind", [cell_fs_str, "/".to_string()]));
    // Read-only base system.
    for (src, dst) in BASE_RO_BINDS {
        arguments.push(Argument::new(
            "--ro-bind",
            [src.to_string(), dst.to_string()],
        ));
    }
    arguments.push(Argument::new("--tmpfs", ["/tmp".to_string()]));
    arguments.push(Argument::new("--proc", ["/proc".to_string()]));
    arguments.push(Argument::new("--dev", ["/dev".to_string()]));
    arguments.push(Argument::new("--unshare-pid", []));
    arguments.push(Argument::new("--unshare-net", []));
    arguments.push(Argument::new("--die-with-parent", []));
    arguments.push(Argument::new("--chdir", [workdir.to_string()]));

    if let Some(bridge) = bridge {
        // Bind-mount the hotcell binary itself read-only as the forwarder
        // supervisor. `current_exe` is the running hotcell; the cell rootfs
        // does not contain it, so we stage it out-of-band (read-only) where
        // the agent cannot modify it.
        let exe = std::env::current_exe().expect("determine hotcell executable path for forwarder");
        let exe_str = exe
            .to_str()
            .expect("hotcell executable path must be valid UTF-8")
            .to_string();
        arguments.push(Argument::new(
            "--ro-bind",
            [exe_str, FORWARDER_BIN.to_string()],
        ));
        // Bind-mount the shared bridge directory read-only so the agent can
        // neither write to nor replace the bridge socket. The forwarder
        // (hotcell-owned) connects to it; the agent never touches it.
        let bridge_host = absolutize(&bridge.host_bridge_dir)
            .to_str()
            .expect("bridge dir path must be valid UTF-8")
            .to_string();
        arguments.push(Argument::new(
            "--ro-bind",
            [bridge_host, BRIDGE_DIR.to_string()],
        ));
    }

    // Clean environment: nothing from the host leaks in. Only the declared
    // env vars are set.
    arguments.push(Argument::new("--clearenv", []));
    for (k, v) in env {
        arguments.push(Argument::new("--setenv", [k.clone(), v.clone()]));
    }

    if let Some(bridge) = bridge {
        // The command bwrap runs: the in-namespace forwarder supervisor,
        // which starts the loopback TCP->UDS relay, discovers its own port,
        // sets HTTP_PROXY/HTTPS_PROXY for the agent, and spawns the agent.
        // The `--` inside the values belongs to the forwarder's own option
        // parsing (its child is the agent program).
        let mut command = vec![
            FORWARDER_BIN.to_string(),
            "fwd".to_string(),
            "--uds".to_string(),
            bridge.uds_in_sandbox.clone(),
            "--".to_string(),
            program.to_string(),
        ];
        command.extend(args.iter().cloned());
        arguments.push(Argument::new("--", command));
    } else {
        let mut command = vec![program.to_string()];
        command.extend(args.iter().cloned());
        arguments.push(Argument::new("--", command));
    }
    arguments
}
