//! Filesystem isolation via bubblewrap (`bwrap`).
//!
//! Mirrors the `FilesystemIsolation` guarantee: the agent cannot access any
//! host filesystem path outside the sandbox. All file operations are confined
//! to the cell's filesystem. The cell's filesystem is backed by the state
//! directory, so agent-produced files live alongside other cell state.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Build the `bwrap` argument list that isolates the agent process.
///
/// The cell's working directory is bind-mounted as the sandbox root's
/// `/work`; host system directories are read-only; `/tmp`, `/proc`, `/dev`
/// are fresh. The program runs inside its own PID namespace and dies with
/// the parent.
pub fn build_command(
    cell_root: &Path,
    workdir: &str,
    env: &[(String, String)],
    program: &str,
    args: &[String],
) -> Command {
    let workdir_path = PathBuf::from(workdir);
    let cell_root_str = cell_root
        .to_str()
        .expect("cell root path must be valid UTF-8");
    let workdir_str = workdir_path
        .to_str()
        .expect("workdir path must be valid UTF-8");

    let mut cmd = Command::new("bwrap");
    cmd.args([
        "--ro-bind", "/usr", "/usr",
        "--ro-bind", "/lib", "/lib",
        "--ro-bind", "/lib64", "/lib64",
        "--ro-bind", "/bin", "/bin",
        "--ro-bind", "/etc", "/etc",
        // The cell's own filesystem: read-write, backed by the state dir.
        "--bind", cell_root_str, cell_root_str,
        // Working directory bind-mounted at /work inside the sandbox.
        "--bind", workdir_str, "/work",
        "--tmpfs", "/tmp",
        "--proc", "/proc",
        "--dev", "/dev",
        "--unshare-pid",
        "--die-with-parent",
        "--chdir", "/work",
    ]);

    for (key, value) in env {
        cmd.args(["--setenv", key, value]);
    }

    cmd.arg(program);
    cmd.args(args);
    cmd
}
