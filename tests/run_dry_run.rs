//! Integration tests for the plan display and run gating: `--dry-run` prints
//! the exact commands hotcell would run without running anything; `-y` /
//! `--no-confirm` skips the confirmation prompt (the plan is still shown);
//! and the confirmation prompt gates real execution.

use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};

/// Path to the built hotcell binary.
fn hotcell_bin() -> String {
    env!("CARGO_BIN_EXE_hotcell").to_string()
}

/// `--dry-run` prints the plan (copy-pasteable, one arg per line with
/// backslash continuations, sandboxed command on a single trailing line),
/// exits 0, and creates no state.
#[test]
fn dry_run_prints_plan_and_creates_no_state() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("Cellfile"), "").expect("write Cellfile");

    let output = Command::new(hotcell_bin())
        .current_dir(dir.path())
        .args(["run", "--dry-run", "--", "/bin/echo", "hello world"])
        .output()
        .expect("run hotcell");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "dry-run should exit 0: {stderr}");
    assert!(stderr.contains("=== agent bwrap ==="), "stderr: {stderr}");
    assert!(stderr.contains("bwrap \\"), "stderr: {stderr}");
    // A flag and its values stay on one line.
    assert!(
        stderr.contains("  --ro-bind /usr /usr \\"),
        "stderr: {stderr}"
    );
    assert!(stderr.contains("  --chdir /work \\"), "stderr: {stderr}");
    // The separator gets its own continuation line; the sandboxed command
    // and its args follow on one final unbroken line, POSIX-quoted.
    assert!(stderr.contains("  -- \\"), "stderr: {stderr}");
    assert!(
        stderr.contains("  /bin/echo 'hello world'"),
        "stderr: {stderr}"
    );
    // Nothing executed, nothing provisioned.
    assert!(
        !dir.path().join(".cell").exists(),
        "dry-run must not create cell state"
    );
}

/// For a networked cell the displayed agent command is the REAL command:
/// the forwarder supervisor is the in-sandbox program and the bridge socket
/// path is visible, not an approximation.
#[test]
fn dry_run_networked_shows_real_forwarder_command() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("Cellfile"),
        "net {\n    allow \"api.example.com\" port=443\n}\n",
    )
    .expect("write Cellfile");

    let output = Command::new(hotcell_bin())
        .current_dir(dir.path())
        .args(["run", "--dry-run", "--", "/bin/true"])
        .output()
        .expect("run hotcell");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");
    assert!(stderr.contains("/hotcell-fwd"), "stderr: {stderr}");
    assert!(stderr.contains("--uds"), "stderr: {stderr}");
    assert!(
        stderr.contains("/hotcell-bridge/proxy.sock"),
        "stderr: {stderr}"
    );
    assert!(stderr.contains("--ro-bind"), "stderr: {stderr}");
}

/// `-y`/`--no-confirm` still prints the plan but skips the prompt and runs.
#[test]
fn no_confirm_runs_without_prompting() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("Cellfile"), "").expect("write Cellfile");

    let output = Command::new(hotcell_bin())
        .current_dir(dir.path())
        .args(["run", "-y", "--", "/bin/echo", "hello"])
        .output()
        .expect("run hotcell");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");
    assert_eq!(stdout.trim_end(), "hello", "stdout: {stdout}");
    assert!(stderr.contains("=== agent bwrap ==="), "stderr: {stderr}");
    assert!(
        !stderr.contains("Run this sandbox?"),
        "no prompt with -y, stderr: {stderr}"
    );
}

/// Declining the prompt aborts before anything runs: exit 1, no state.
#[test]
fn prompt_aborts_on_n_without_creating_state() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("Cellfile"), "").expect("write Cellfile");

    let mut child = Command::new(hotcell_bin())
        .current_dir(dir.path())
        .args(["run", "--", "/bin/echo", "should-not-run"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn hotcell");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"n\n")
        .expect("write n");
    let output = child.wait_with_output().expect("run hotcell");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "declined run should fail");
    assert!(stderr.contains("run aborted"), "stderr: {stderr}");
    assert!(
        !dir.path().join(".cell").exists(),
        "aborted run must not provision"
    );
}

/// Confirming with `y` proceeds into the real run.
#[test]
fn prompt_proceeds_on_y() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("Cellfile"), "").expect("write Cellfile");

    let mut child = Command::new(hotcell_bin())
        .current_dir(dir.path())
        .args(["run", "--", "/bin/echo", "confirmed"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn hotcell");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"y\n")
        .expect("write y");
    let output = child.wait_with_output().expect("run hotcell");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "confirmed run should succeed");
    assert_eq!(stdout.trim_end(), "confirmed", "stdout: {stdout}");
}

/// A shell-provisioned cell that is already current shows the provisioning
/// step as skipped in the plan — the displayed plan matches the executed run
/// exactly (no provisioning command is shown that would never run).
#[test]
fn dry_run_omits_provisioning_step_when_cell_current() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("Cellfile"),
        "provision \"shell\" script=\"./p.sh\"\nworkdir \"/work\"\n",
    )
    .expect("write Cellfile");
    fs::write(dir.path().join("p.sh"), "#!/bin/sh\nexit 0\n").expect("write p.sh");
    fs::set_permissions(dir.path().join("p.sh"), PermissionsExt::from_mode(0o755))
        .expect("chmod p.sh");

    // Provision once for real.
    let run = Command::new(hotcell_bin())
        .current_dir(dir.path())
        .args(["run", "-y", "--", "/bin/true"])
        .output()
        .expect("run hotcell");
    assert!(
        run.status.success(),
        "first run failed: {}",
        String::from_utf8_lossy(&run.stderr)
    );

    // Now a dry run must NOT show a provisioning step (it would be skipped).
    let output = Command::new(hotcell_bin())
        .current_dir(dir.path())
        .args(["run", "--dry-run", "--", "/bin/true"])
        .output()
        .expect("run hotcell");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");
    assert!(
        !stderr.contains("=== provisioning bwrap ==="),
        "stderr: {stderr}"
    );
    assert!(stderr.contains("provisioning: skipped"), "stderr: {stderr}");
}

/// A shell-provisioned cell that is NOT yet provisioned shows the real
/// provisioning bwrap step in the plan.
#[test]
fn dry_run_shows_provisioning_step_for_shell_cell() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("Cellfile"),
        "provision \"shell\" script=\"./p.sh\"\n",
    )
    .expect("write Cellfile");
    fs::write(dir.path().join("p.sh"), "#!/bin/sh\nexit 0\n").expect("write p.sh");
    fs::set_permissions(dir.path().join("p.sh"), PermissionsExt::from_mode(0o755))
        .expect("chmod p.sh");

    let output = Command::new(hotcell_bin())
        .current_dir(dir.path())
        .args(["run", "--dry-run", "--", "/bin/true"])
        .output()
        .expect("run hotcell");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");
    assert!(
        stderr.contains("=== provisioning bwrap ==="),
        "stderr: {stderr}"
    );
    assert!(
        stderr.contains("/hotcell/cellfile/p.sh"),
        "the staged script path must be what really runs: {stderr}"
    );
}

/// Config errors surface from plan building, before any display or prompt:
/// a shell provisioner without a script path fails fast with a clear
/// message and creates no state.
#[test]
fn missing_script_fails_before_prompt() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("Cellfile"), "provision \"shell\"\n").expect("write Cellfile");

    let output = Command::new(hotcell_bin())
        .current_dir(dir.path())
        .args(["run", "-y", "--", "/bin/true"])
        .output()
        .expect("run hotcell");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "stderr: {stderr}");
    assert!(stderr.contains("requires a `script="), "stderr: {stderr}");
    assert!(
        !dir.path().join(".cell").exists(),
        "no state on config error"
    );
}

/// Relative host paths — a relative `--file`, or a relative
/// `host-path` — are absolutized in the plan, so the displayed
/// command is unambiguous and resolves identically when copied and pasted
/// from any directory.
#[test]
fn dry_run_absolutizes_relative_paths() {
    let dir = tempfile::tempdir().unwrap();
    let sub = dir.path().join("sub");
    fs::create_dir_all(&sub).expect("create sub");
    fs::write(
        sub.join("Cellfile"),
        "provision \"shell\" script=\"./p.sh\" {\n    host-path \"./tools\"\n}\n",
    )
    .expect("write Cellfile");
    fs::write(sub.join("p.sh"), "#!/bin/sh\nexit 0\n").expect("write p.sh");
    fs::set_permissions(sub.join("p.sh"), PermissionsExt::from_mode(0o755)).expect("chmod p.sh");

    // cwd is `dir`; the Cellfile is named relative: `sub/Cellfile`.
    let output = Command::new(hotcell_bin())
        .current_dir(dir.path())
        .args([
            "run",
            "--dry-run",
            "--file",
            "sub/Cellfile",
            "--",
            "/bin/true",
        ])
        .output()
        .expect("run hotcell");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");
    let cwd = dir.path().display().to_string();

    // The rootfs bind source is absolute, with the `.` component dropped.
    let bind_abs = format!("--bind {cwd}/sub/.cell/default/fs /");
    assert!(stderr.contains(&bind_abs), "stderr: {stderr}");
    assert!(
        !stderr.contains("--bind sub/") && !stderr.contains("--bind ./sub"),
        "relative bind must not leak into the plan: {stderr}"
    );

    // The relative host_path is absolutized, and its staged target mirrors
    // the absolute form (no `/hotcell/host./tools` mangling).
    let host_abs = format!("--ro-bind {cwd}/tools /hotcell/host{cwd}/tools");
    assert!(stderr.contains(&host_abs), "stderr: {stderr}");

    // The cellfile directory staged for the provisioner is absolute too.
    let cellfile_abs = format!("--ro-bind {cwd}/sub /hotcell/cellfile");
    assert!(stderr.contains(&cellfile_abs), "stderr: {stderr}");
}
