//! Integration test: the `--provision KIND[:FILE]` CLI override.
//!
//! Covers overriding the Cellfile's declared provisioner from the command
//! line for a single run. The override selects the provisioner kind and
//! (for `script`) the script path, while preserving the Cellfile's declared
//! `provision.host_paths` bootstrap allowance.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

use tempfile::TempDir;

fn hotcell_bin() -> String {
    env!("CARGO_BIN_EXE_hotcell").to_string()
}

/// `--provision script:provision.sh` overrides a Cellfile that declared no
/// provisioner (defaulting to `none`). The script provisions a tool the agent
/// then runs.
#[test]
fn provision_override_runs_script_provisioner() {
    let dir = TempDir::new().expect("create temp dir");

    // Cellfile with no provisioner declared — defaults to "none".
    fs::write(dir.path().join("Cellfile"), "workdir = /work\n").unwrap();

    fs::write(
        dir.path().join("provision.sh"),
        "#!/usr/bin/env bash\nset -euo pipefail\n\
         mkdir -p \"$HOTCELL_CELL_ROOT/opt/bin\" \"$HOTCELL_WORKDIR_HOST\"\n\
         printf '#!/usr/bin/env bash\\necho overridden\\n' \\\n\
           > \"$HOTCELL_CELL_ROOT/opt/bin/hello\"\n\
         chmod +x \"$HOTCELL_CELL_ROOT/opt/bin/hello\"\n",
    )
    .unwrap();
    fs::set_permissions(
        dir.path().join("provision.sh"),
        PermissionsExt::from_mode(0o755),
    )
    .unwrap();

    let output = Command::new(hotcell_bin())
        .current_dir(dir.path())
        .args(["run", "--provision", "script:./provision.sh", "--", "/opt/bin/hello"])
        .output()
        .expect("run hotcell");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "hotcell run failed: exit {:?}\nstdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );
    assert_eq!(stdout.trim_end(), "overridden");
}

/// `--provision none` overrides a Cellfile that declared `script`. The script
/// is ignored (no provisioning work), and the agent runs with an empty rootfs.
#[test]
fn provision_override_none_overrides_script() {
    let dir = TempDir::new().expect("create temp dir");

    // Cellfile declares the script provisioner, but we override to none.
    fs::write(
        dir.path().join("Cellfile"),
        "provisioner = script\nprovision.script = ./never-run.sh\nworkdir = /work\n",
    )
    .unwrap();

    // A script that would fail if run (proving it was not invoked).
    fs::write(
        dir.path().join("never-run.sh"),
        "#!/usr/bin/env bash\nexit 99\n",
    )
    .unwrap();
    fs::set_permissions(
        dir.path().join("never-run.sh"),
        PermissionsExt::from_mode(0o755),
    )
    .unwrap();

    let output = Command::new(hotcell_bin())
        .current_dir(dir.path())
        .args(["run", "--provision", "none", "--", "/bin/pwd"])
        .output()
        .expect("run hotcell");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "hotcell run failed: exit {:?}\nstdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );
    assert_eq!(stdout.trim_end(), "/work");
}

/// The override preserves the Cellfile's declared `provision.host_paths`: the
/// overridden script provisioner can still read a host path the Cellfile
/// declared, even though the override only named the kind and script.
#[test]
fn provision_override_preserves_host_paths() {
    let dir = TempDir::new().expect("create temp dir");
    let host_tools = TempDir::new().expect("create host tools dir");
    fs::write(host_tools.path().join("marker.txt"), "from-host\n").unwrap();

    // Cellfile declares host_paths but no script provisioner. We override to
    // script on the CLI; the host_paths must still be honoured.
    fs::write(
        dir.path().join("Cellfile"),
        format!(
            "provision.host_path = {host}\nworkdir = /work\n",
            host = host_tools.path().display()
        ),
    )
    .unwrap();

    fs::write(
        dir.path().join("provision.sh"),
        format!(
            "#!/usr/bin/env bash\nset -euo pipefail\n\
             mkdir -p \"$HOTCELL_CELL_ROOT/opt/bin\" \"$HOTCELL_WORKDIR_HOST\"\n\
             cp \"${{HOTCELL_HOST_ROOT}}{host}/marker.txt\" \"$HOTCELL_CELL_ROOT/opt/bin/marker.txt\"\n\
             printf '#!/usr/bin/env bash\\ncat /opt/bin/marker.txt\\n' \\\n\
               > \"$HOTCELL_CELL_ROOT/opt/bin/show\"\n\
             chmod +x \"$HOTCELL_CELL_ROOT/opt/bin/show\"\n",
            host = host_tools.path().display()
        ),
    )
    .unwrap();
    fs::set_permissions(
        dir.path().join("provision.sh"),
        PermissionsExt::from_mode(0o755),
    )
    .unwrap();

    let output = Command::new(hotcell_bin())
        .current_dir(dir.path())
        .args(["run", "--provision", "script:./provision.sh", "--", "/opt/bin/show"])
        .output()
        .expect("run hotcell");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "hotcell run failed: exit {:?}\nstdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );
    assert_eq!(stdout.trim_end(), "from-host");
}
