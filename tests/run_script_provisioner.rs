//! Integration test: the script provisioner provisions a tool into the cell's
//! rootfs, then the agent runs it inside the sandbox.
//!
//! Covers the `ProvisionedEnvironment` guarantee: the agent's tools come from
//! the provisioned rootfs (bind-mounted as `/`), not from host bind-mounts.
//! The provision script installs a small executable under `/opt/bin` inside
//! the cell root and prints a marker; `hotcell run` then executes it by its
//! in-sandbox path and relays the output.

use std::fs;
use std::process::Command;

use tempfile::TempDir;

fn hotcell_bin() -> String {
    env!("CARGO_BIN_EXE_hotcell").to_string()
}

#[test]
fn script_provisioner_installs_tool_visible_in_sandbox() {
    let dir = TempDir::new().expect("create temp dir");

    // A Cellfile that selects the script provisioner.
    fs::write(
        dir.path().join("Cellfile"),
        "provisioner = script\nprovision.script = ./provision.sh\nworkdir = /work\n",
    )
    .expect("write Cellfile");

    // A provision script that installs a tool under /opt/bin (a path not
    // shadowed by the read-only base-system binds) and creates the workdir.
    // It also records the HOTCELL_* env vars it received, so we can assert the
    // provisioner contract is honoured.
    fs::write(
        dir.path().join("provision.sh"),
        "#!/usr/bin/env bash\nset -euo pipefail\n\
         mkdir -p \"$HOTCELL_CELL_ROOT/opt/bin\" \"$HOTCELL_WORKDIR_HOST\"\n\
         printf '#!/usr/bin/env bash\\necho provisioned-marker\\n' > \"$HOTCELL_CELL_ROOT/opt/bin/hello\"\n\
         chmod +x \"$HOTCELL_CELL_ROOT/opt/bin/hello\"\n\
         # prove the contract env vars are present\n\
         test -n \"$HOTCELL_CELL_ROOT\"\n\
         test -n \"$HOTCELL_CELLFILE_DIR\"\n\
         test \"$HOTCELL_WORKDIR\" = \"/work\"\n",
    )
    .expect("write provision.sh");
    fs::set_permissions(
        dir.path().join("provision.sh"),
        std::os::unix::fs::PermissionsExt::from_mode(0o755),
    )
    .expect("chmod provision.sh");

    let output = Command::new(hotcell_bin())
        .current_dir(dir.path())
        .args(["run", "/opt/bin/hello"])
        .output()
        .expect("run hotcell");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "hotcell run failed: exit {:?}\nstdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );
    assert_eq!(
        stdout.trim_end(),
        "provisioned-marker",
        "stdout: {stdout}\nstderr: {stderr}"
    );
}

#[test]
fn script_provisioner_failure_fails_provisioning() {
    let dir = TempDir::new().expect("create temp dir");

    fs::write(
        dir.path().join("Cellfile"),
        "provisioner = script\nprovision.script = ./provision.sh\n",
    )
    .expect("write Cellfile");

    // A provision script that exits nonzero.
    fs::write(
        dir.path().join("provision.sh"),
        "#!/usr/bin/env bash\necho 'installing things...' >&2\nexit 3\n",
    )
    .expect("write provision.sh");
    fs::set_permissions(
        dir.path().join("provision.sh"),
        std::os::unix::fs::PermissionsExt::from_mode(0o755),
    )
    .expect("chmod provision.sh");

    let output = Command::new(hotcell_bin())
        .current_dir(dir.path())
        .args(["run", "/bin/true"])
        .output()
        .expect("run hotcell");

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Provisioning failed: hotcell exits 1 and reports the failure.
    assert_eq!(
        output.status.code(),
        Some(1),
        "expected provisioning failure (exit 1), got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("provisioning failed"),
        "stderr should report provisioning failure: {stderr}"
    );
    assert!(
        stderr.contains("exited with code 3"),
        "stderr should report the script exit code: {stderr}"
    );
}

#[test]
fn script_provisioner_without_script_path_is_config_error() {
    let dir = TempDir::new().expect("create temp dir");

    // Selects the script provisioner but omits provision.script.
    fs::write(
        dir.path().join("Cellfile"),
        "provisioner = script\n",
    )
    .expect("write Cellfile");

    let output = Command::new(hotcell_bin())
        .current_dir(dir.path())
        .args(["run", "/bin/true"])
        .output()
        .expect("run hotcell");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(1),
        "expected a hard config error (exit 1), got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("provision.script"),
        "expected a clear error about the missing script path: {stderr}"
    );
}
