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
//! | Declared host paths   | read-only binds      | **none**      |
//! | Cellfile directory    | read-only bind       | **none**      |
//! | Network               | shared (egress)      | offline/firewall |
//! | Environment           | host env + controls  | clean (declared only) |
//!
//! The provisioner may bootstrap (read host tools, fetch packages); the agent
//! may not. Neither can write to the host — all host binds are read-only.
//! Mirrors the `ProvisionerRunsInCell`, `BootstrapRiskProfile`,
//! `ProvisionerCannotWriteHost`, `ProvisionerCleanEnvironment`,
//! `FilesystemIsolation`, and `ProvisionedEnvironment` guarantees.

use std::path::{Path, PathBuf};
use std::process::Command;

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
/// directory and the declared host bootstrap paths, shared network, and the
/// host environment plus the given control/env vars. `script` is the program
/// to execute (resolved by the caller to an absolute path).
pub fn build_provisioning_command(
    cell_fs: &Path,
    cellfile_directory: &Path,
    host_paths: &[String],
    control_env: &[(String, String)],
    script: &Path,
) -> Command {
    let cell_fs_str = cell_fs
        .to_str()
        .expect("cell rootfs path must be valid UTF-8");
    let cellfile_dir_str = cellfile_directory
        .to_str()
        .expect("cellfile directory must be valid UTF-8");

    let mut cmd = Command::new("bwrap");
    // Cell rootfs as the sandbox root, read-write.
    cmd.args(["--bind", cell_fs_str, "/"]);
    // Read-only base system.
    for (src, dst) in BASE_RO_BINDS {
        cmd.args(["--ro-bind", src, dst]);
    }
    // Fresh /tmp, /proc, /dev — created before the cellfile/host binds below
    // so that a cellfile directory living under /tmp is not masked by the
    // tmpfs (bwrap applies operations left-to-right).
    cmd.args(["--tmpfs", "/tmp", "--proc", "/proc", "--dev", "/dev"]);
    // The Cellfile directory, read-only at the same path, so the provision
    // script and any project-relative seeds resolve inside the sandbox.
    cmd.args(["--ro-bind", cellfile_dir_str, cellfile_dir_str]);
    // Declared host bootstrap paths, read-only at the same path. These are the
    // provisioner's privilege; the agent never sees them.
    for hp in host_paths {
        let resolved = expand_home(hp);
        if let Some(s) = resolved.to_str() {
            cmd.args(["--ro-bind", s, s]);
        }
    }
    cmd.args([
        "--unshare-pid",
        "--share-net",
        "--die-with-parent",
        "--chdir", cellfile_dir_str,
    ]);

    // The provisioner inherits the host environment (so standard tooling —
    // node, npm, curl, git — works during bootstrap). HOTCELL_* control vars
    // and declared agent env are layered on top via --setenv.
    for (k, v) in control_env {
        cmd.args(["--setenv", k, v]);
    }

    cmd.arg(script);
    cmd
}

/// Build the **agent** profile command: the agent runs inside the cell rootfs
/// (`cell_fs` as `/`) with *no* host path access, a clean environment
/// containing only the declared `env` vars, and no network access.
///
/// The network firewall (HTTP allowlist proxy) is not yet implemented; until
/// it is, the agent is always offline. The caller must reject Cellfiles that
/// declare a non-empty network policy rather than silently granting egress.
pub fn build_agent_command(
    cell_fs: &Path,
    workdir: &str,
    env: &[(String, String)],
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
        "--tmpfs", "/tmp",
        "--proc", "/proc",
        "--dev", "/dev",
        "--unshare-pid",
        "--unshare-net",
        "--die-with-parent",
        "--chdir", &workdir_str,
    ]);

    // Clean environment: nothing from the host leaks in. Only the declared
    // env vars are set.
    cmd.arg("--clearenv");
    for (k, v) in env {
        cmd.args(["--setenv", k, v]);
    }

    cmd.arg(program);
    cmd.args(args);
    cmd
}
