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
//! Host paths (the Cellfile directory and `provision.host_path` entries) are
//! bind-mounted under a neutral staging prefix ([`HOST_STAGING_DIR`], `/hotcell`)
//! rather than at their original absolute host paths. This is critical: bwrap
//! creates intermediate directories in the rootfs for bind targets whose paths
//! do not already exist, and those stubs persist after the sandbox exits. If
//! host paths were bound at their original locations (e.g. `/home/user/.pi`),
//! the rootfs would inherit a `/home/user/...` skeleton that leaks the host
//! username and directory structure to the agent. Staging under `/hotcell`
//! confines all such stubs to a single directory that the caller removes from
//! the rootfs after provisioning completes.

use std::path::{Path, PathBuf};
use std::process::Command;

/// In-sandbox mount point for the bind-mounted hotcell binary, used as the
/// forwarder supervisor when a network bridge is active. Lives outside the
/// cell rootfs proper so the agent cannot see or tamper with the supervisor
/// binary (the bind is read-only).
const FORWARDER_BIN: &str = "/hotcell-fwd";

/// In-sandbox mount point for the shared Unix-socket bridge directory.
/// Bind-mounted read-only from a cell-scoped host directory so the agent can
/// neither write to nor replace the bridge socket (it can only be connected
/// to by the in-namespace forwarder, which is hotcell-owned code).
const BRIDGE_DIR: &str = "/hotcell-bridge";

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
pub fn build_provisioning_command(
    cell_fs: &Path,
    cellfile_directory: &Path,
    host_paths: &[String],
    control_env: &[(String, String)],
    script_rel: &str,
) -> Command {
    let cell_fs_str = cell_fs
        .to_str()
        .expect("cell rootfs path must be valid UTF-8");
    let cellfile_dir_str = cellfile_directory
        .to_str()
        .expect("cellfile directory must be valid UTF-8");

    let staging = HOST_STAGING_DIR;
    let cellfile_mount = format!("{staging}/cellfile");
    let host_root = format!("{staging}/host");
    let script_in_sandbox = {
        let rel = script_rel.trim_start_matches("./");
        format!("{cellfile_mount}/{rel}")
    };

    let mut cmd = Command::new("bwrap");
    // Cell rootfs as the sandbox root, read-write.
    cmd.args(["--bind", cell_fs_str, "/"]);
    // Read-only base system.
    for (src, dst) in BASE_RO_BINDS {
        cmd.args(["--ro-bind", src, dst]);
    }
    // Fresh /tmp, /proc, /dev — before the staged binds below so that a
    // Cellfile/host path under /tmp is not masked (bwrap applies ops in order).
    cmd.args(["--tmpfs", "/tmp", "--proc", "/proc", "--dev", "/dev"]);

    // Stage the Cellfile directory read-only under the staging prefix so the
    // provision script and project-relative seeds resolve without leaving a
    // host-path skeleton in the rootfs.
    cmd.args(["--ro-bind", cellfile_dir_str, &cellfile_mount]);

    // Stage each declared host bootstrap path read-only under
    // {staging}/host/<original absolute path>. These are the provisioner's
    // privilege; the agent never sees them.
    for hp in host_paths {
        let resolved = expand_home(hp);
        if let Some(s) = resolved.to_str() {
            let target = format!("{host_root}{s}");
            cmd.args(["--ro-bind", s, &target]);
        }
    }

    cmd.args([
        "--unshare-pid",
        "--share-net",
        "--die-with-parent",
        "--chdir",
        &cellfile_mount,
    ]);

    // Staging variables (encapsulate the staging scheme here).
    let host_home = std::env::var("HOME").unwrap_or_default();
    let host_home_staged = format!("{host_root}{host_home}");
    cmd.args(["--setenv", "HOTCELL_CELLFILE_DIR", &cellfile_mount]);
    cmd.args(["--setenv", "HOTCELL_HOST_ROOT", &host_root]);
    cmd.args(["--setenv", "HOTCELL_HOST_HOME", &host_home_staged]);

    // Caller-supplied control/env vars (HOTCELL_*, HOME override, etc.).
    for (k, v) in control_env {
        cmd.args(["--setenv", k, v]);
    }

    cmd.arg(&script_in_sandbox);
    cmd
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
pub fn build_agent_command(
    cell_fs: &Path,
    workdir: &str,
    env: &[(String, String)],
    bridge: Option<&AgentBridge>,
    program: &str,
    args: &[String],
) -> Command {
    let cell_fs_str = cell_fs
        .to_str()
        .expect("cell rootfs path must be valid UTF-8");
    let workdir_str = workdir.to_string();

    let mut cmd = Command::new("bwrap");
    // Cell rootfs as the sandbox root.
    cmd.args(["--bind", cell_fs_str, "/"]);
    // Read-only base system.
    for (src, dst) in BASE_RO_BINDS {
        cmd.args(["--ro-bind", src, dst]);
    }
    cmd.args([
        "--tmpfs",
        "/tmp",
        "--proc",
        "/proc",
        "--dev",
        "/dev",
        "--unshare-pid",
        "--unshare-net",
        "--die-with-parent",
        "--chdir",
        &workdir_str,
    ]);

    if let Some(bridge) = bridge {
        // Bind-mount the hotcell binary itself read-only as the forwarder
        // supervisor. `current_exe` is the running hotcell; the cell rootfs
        // does not contain it, so we stage it out-of-band (read-only) where
        // the agent cannot modify it.
        let exe = std::env::current_exe().expect("determine hotcell executable path for forwarder");
        let exe_str = exe
            .to_str()
            .expect("hotcell executable path must be valid UTF-8");
        cmd.args(["--ro-bind", exe_str, FORWARDER_BIN]);
        // Bind-mount the shared bridge directory read-only so the agent can
        // neither write to nor replace the bridge socket. The forwarder
        // (hotcell-owned) connects to it; the agent never touches it.
        let bridge_host = bridge
            .host_bridge_dir
            .to_str()
            .expect("bridge dir path must be valid UTF-8");
        cmd.args(["--ro-bind", bridge_host, BRIDGE_DIR]);
    }

    // Clean environment: nothing from the host leaks in. Only the declared
    // env vars are set.
    cmd.arg("--clearenv");
    for (k, v) in env {
        cmd.args(["--setenv", k, v]);
    }

    if let Some(bridge) = bridge {
        // Launch the in-namespace forwarder supervisor, which starts the
        // loopback TCP->UDS relay, discovers its own port, sets
        // HTTP_PROXY/HTTPS_PROXY for the agent, and spawns the agent.
        cmd.arg(FORWARDER_BIN);
        cmd.args(["fwd", "--uds", &bridge.uds_in_sandbox, "--", program]);
        cmd.args(args);
    } else {
        cmd.arg(program);
        cmd.args(args);
    }
    cmd
}
