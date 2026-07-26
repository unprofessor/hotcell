//! Integration test: the provisioner and agent have distinct risk profiles.
//!
//! Covers the `BootstrapRiskProfile` and `ProvisionedEnvironment` guarantees:
//! the provisioner runs inside the cell with read-only access to declared
//! host paths (so it can bootstrap tools), while the agent runs with *no*
//! host-path access. Both are confined to the cell rootfs for writes.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

use tempfile::TempDir;

fn hotcell_bin() -> String {
    env!("CARGO_BIN_EXE_hotcell").to_string()
}

/// The provisioner can read a declared `provision.host_path` and copy a tool
/// from it into the cell rootfs; the agent then runs that tool, proving the
/// provisioner's bootstrap allowance worked.
#[test]
fn provisioner_can_read_declared_host_path() {
    let dir = TempDir::new().expect("create temp dir");

    // A "host tool" directory outside the cellfile dir, with a marker file.
    let host_tools = TempDir::new().expect("create host tools dir");
    fs::write(host_tools.path().join("marker.txt"), "host-secret\n").unwrap();

    fs::write(
        dir.path().join("Cellfile"),
        format!(
            "provisioner = script\n\
             provision.script = ./provision.sh\n\
             provision.host_path = {host}\n\
             workdir = /work\n",
            host = host_tools.path().display()
        ),
    )
    .expect("write Cellfile");

    // The provision script copies the marker from the host path (visible to
    // the provisioner) into the cell rootfs, then makes a tiny program that
    // prints it.
    fs::write(
        dir.path().join("provision.sh"),
        format!(
            "#!/usr/bin/env bash\nset -euo pipefail\n\
             mkdir -p \"$HOTCELL_CELL_ROOT/opt/bin\" \"$HOTCELL_WORKDIR_HOST\"\n\
             cp \"${{HOTCELL_HOST_ROOT}}{host}/marker.txt\" \"$HOTCELL_CELL_ROOT/opt/bin/marker.txt\"\n\
             printf '#!/usr/bin/env bash\\ncat /opt/bin/marker.txt\\n' \\\n\
               > \"$HOTCELL_CELL_ROOT/opt/bin/show-marker\"\n\
             chmod +x \"$HOTCELL_CELL_ROOT/opt/bin/show-marker\"\n",
            host = host_tools.path().display()
        ),
    )
    .expect("write provision.sh");
    fs::set_permissions(
        dir.path().join("provision.sh"),
        PermissionsExt::from_mode(0o755),
    )
    .unwrap();

    let output = Command::new(hotcell_bin())
        .current_dir(dir.path())
        .args(["run", "/opt/bin/show-marker"])
        .output()
        .expect("run hotcell");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "hotcell run failed: exit {:?}\nstdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );
    assert_eq!(stdout.trim_end(), "host-secret");
}

/// The agent cannot see the host path the provisioner used: a marker left at
/// the host path's in-sandbox location is absent when the agent looks for it.
/// This proves the host-path binds are provisioner-only.
#[test]
fn agent_cannot_see_provisioner_host_paths() {
    let dir = TempDir::new().expect("create temp dir");

    let host_tools = TempDir::new().expect("create host tools dir");
    let host_marker = host_tools.path().join("secret.txt");
    fs::write(&host_marker, "should-not-leak\n").unwrap();

    fs::write(
        dir.path().join("Cellfile"),
        format!(
            "provisioner = script\n\
             provision.script = ./provision.sh\n\
             provision.host_path = {host}\n\
             workdir = /work\n",
            host = host_tools.path().display()
        ),
    )
    .expect("write Cellfile");

    // The provision script creates the workdir but does NOT copy the host
    // marker anywhere. The agent will then try to read the host path at its
    // original location — which must not be visible.
    fs::write(
        dir.path().join("provision.sh"),
        "#!/usr/bin/env bash\nset -euo pipefail\nmkdir -p \"$HOTCELL_WORKDIR_HOST\"\n",
    )
    .expect("write provision.sh");
    fs::set_permissions(
        dir.path().join("provision.sh"),
        PermissionsExt::from_mode(0o755),
    )
    .unwrap();

    // Agent: `test -f <host_marker>` should fail (file not visible), and we
    // assert a nonzero exit. We use `sh -c` so we can express the test.
    let agent_check = format!("test -f {}", host_marker.display());
    let output = Command::new(hotcell_bin())
        .current_dir(dir.path())
        .args(["run", "--", "/bin/sh", "-c", &agent_check])
        .output()
        .expect("run hotcell");

    let stderr = String::from_utf8_lossy(&output.stderr);
    // The agent must NOT see the host marker: `test -f` exits 1.
    assert_eq!(
        output.status.code(),
        Some(1),
        "agent unexpectedly saw the host path (should be invisible): \
         exit {:?}\nstderr: {stderr}",
        output.status.code()
    );
}

/// The agent runs with a clean environment: no host environment variables
/// leak in, only the declared `env.*` vars are present.
#[test]
fn agent_has_clean_environment() {
    let dir = TempDir::new().expect("create temp dir");

    // Use the none provisioner (no script needed).
    fs::write(
        dir.path().join("Cellfile"),
        "workdir = /work\nenv.HOTCELL_TEST_VAR = provisioned\n",
    )
    .expect("write Cellfile");

    // Set a unique host env var that must NOT appear inside the cell.
    let host_leak = "HOTCELL_HOST_LEAK_SHOULD_NOT_APPEAR";
    let agent_check = format!(
        "test \"$HOTCELL_TEST_VAR\" = provisioned && test -z \"${host_leak}\""
    );

    let output = Command::new(hotcell_bin())
        .current_dir(dir.path())
        .env(host_leak, "leaked")
        .args(["run", "--", "/bin/sh", "-c", &agent_check])
        .output()
        .expect("run hotcell");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "agent env was not clean: exit {:?}\nstdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );
}

/// Regression: provisioning must not leave a skeleton of the host user's home
/// directory in the cell rootfs. Two mechanisms caused this historically:
/// (1) the provisioner inheriting HOME=/home/<user>, so tools like npm cached
/// into $HOME inside the rootfs; (2) bwrap creating intermediate directories
/// in the rootfs for host-path binds whose targets lived under /home/<user>.
/// Both are fixed: HOME is overridden to a cell-local path, and host paths are
/// staged under /hotcell and cleaned up after provisioning. This test declares
/// host paths under the temp dir (which stands in for the host home) and
/// asserts the agent sees neither the host home path nor the staging prefix.
#[test]
fn host_home_does_not_leak_into_cell() {
    let dir = TempDir::new().expect("create temp dir");

    // A fake "host home" with a config dir, standing in for ~/.pi etc.
    let host_home = TempDir::new().expect("create host home");
    fs::write(host_home.path().join(".gitconfig"), "[user]\n").unwrap();
    fs::create_dir(host_home.path().join(".pi")).unwrap();

    fs::write(
        dir.path().join("Cellfile"),
        format!(
            "provisioner = script\n\
             provision.script = ./provision.sh\n\
             provision.host_path = {home}/.pi\n\
             provision.host_path = {home}/.gitconfig\n\
             workdir = /work\n",
            home = host_home.path().display()
        ),
    )
    .expect("write Cellfile");

    // The provision script touches the staged host paths (proving they were
    // visible to the provisioner) and creates the workdir.
    fs::write(
        dir.path().join("provision.sh"),
        format!(
            "#!/usr/bin/env bash\nset -euo pipefail\n\
             test -f \"${{HOTCELL_HOST_ROOT}}{home}/.gitconfig\"\n\
             test -d \"${{HOTCELL_HOST_ROOT}}{home}/.pi\"\n\
             mkdir -p \"$HOTCELL_WORKDIR_HOST\"\n",
            home = host_home.path().display()
        ),
    )
    .expect("write provision.sh");
    fs::set_permissions(
        dir.path().join("provision.sh"),
        PermissionsExt::from_mode(0o755),
    )
    .unwrap();

    // The agent must NOT see the host home path nor the staging prefix. Both
    // `test -e` checks must fail (exit 1); we invert with `!` so success means
    // "neither path exists".
    let home_in_cell = host_home.path().display().to_string();
    let check = format!(
        "test ! -e {home} && test ! -e /hotcell",
        home = home_in_cell
    );

    let output = Command::new(hotcell_bin())
        .current_dir(dir.path())
        .args(["run", "--", "/bin/sh", "-c", &check])
        .output()
        .expect("run hotcell");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "host home or staging prefix leaked into the cell: exit {:?}\n\
         stdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );
}
