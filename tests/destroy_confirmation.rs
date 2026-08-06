//! Integration test: `hotcell destroy` asks for confirmation before tearing
//! down a cell (`[y/N]`, default no), and `--force` bypasses the prompt.
//! Declining — or a non-interactive stdin hitting EOF — leaves the cell's
//! state directory untouched.

use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

/// Path to the built hotcell binary.
fn hotcell_bin() -> String {
    env!("CARGO_BIN_EXE_hotcell").to_string()
}

/// Provision the default cell with state rooted in `cell_dir`.
fn provision_cell(cwd: &std::path::Path, cell_dir: &std::path::Path) {
    let run = Command::new(hotcell_bin())
        .current_dir(cwd)
        .args([
            "run",
            "--cell",
            cell_dir.to_str().unwrap(),
            "--",
            "/bin/true",
        ])
        .output()
        .expect("run hotcell");
    assert!(run.status.success(), "provisioning run failed");
    assert!(cell_dir.join(".cell/default").exists());
}

/// Spawn `destroy` for the default cell with piped stdio and the given extra
/// args, feed `stdin_text` to it, and collect the output.
fn destroy_with_stdin(
    cwd: &std::path::Path,
    cell_dir: &std::path::Path,
    extra_args: &[&str],
    stdin_text: &str,
) -> std::process::Output {
    let mut cmd = Command::new(hotcell_bin());
    cmd.current_dir(cwd)
        .arg("destroy")
        .args(extra_args)
        .args(["--cell", cell_dir.to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().expect("spawn destroy");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(stdin_text.as_bytes())
        .unwrap();
    child.wait_with_output().expect("destroy hotcell")
}

/// `y` confirms: the cell is destroyed and its state directory removed.
#[test]
fn destroy_confirms_on_y() {
    let cwd = tempfile::tempdir().unwrap();
    let cell_dir = tempfile::tempdir().unwrap();
    fs::write(cwd.path().join("Cellfile"), "workdir = /work\n").unwrap();
    provision_cell(cwd.path(), cell_dir.path());

    let output = destroy_with_stdin(cwd.path(), cell_dir.path(), &[], "y\n");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "destroy after 'y' failed:\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(stdout.contains("destroyed cell"), "stdout: {stdout}");
    assert!(
        !cell_dir.path().join(".cell/default").exists(),
        "cell state should be gone after confirmed destroy"
    );
}

/// `yes` (any case) also confirms.
#[test]
fn destroy_confirms_on_yes_uppercase() {
    let cwd = tempfile::tempdir().unwrap();
    let cell_dir = tempfile::tempdir().unwrap();
    fs::write(cwd.path().join("Cellfile"), "workdir = /work\n").unwrap();
    provision_cell(cwd.path(), cell_dir.path());

    let output = destroy_with_stdin(cwd.path(), cell_dir.path(), &[], "YES\n");

    assert!(
        output.status.success(),
        "destroy after 'YES' failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!cell_dir.path().join(".cell/default").exists());
}

/// `n` declines: the command fails and the cell's state is left untouched.
#[test]
fn destroy_declines_on_n() {
    let cwd = tempfile::tempdir().unwrap();
    let cell_dir = tempfile::tempdir().unwrap();
    fs::write(cwd.path().join("Cellfile"), "workdir = /work\n").unwrap();
    provision_cell(cwd.path(), cell_dir.path());

    let output = destroy_with_stdin(cwd.path(), cell_dir.path(), &[], "n\n");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "destroy after 'n' should fail");
    assert!(stderr.contains("aborted"), "stderr: {stderr}");
    assert!(
        cell_dir.path().join(".cell/default").exists(),
        "cell state should survive a declined destroy"
    );
}

/// EOF on stdin (non-interactive use) is a decline by default: no answer is
/// a no.
#[test]
fn destroy_aborts_on_eof_without_force() {
    let cwd = tempfile::tempdir().unwrap();
    let cell_dir = tempfile::tempdir().unwrap();
    fs::write(cwd.path().join("Cellfile"), "workdir = /work\n").unwrap();
    provision_cell(cwd.path(), cell_dir.path());

    // `.output()` wires stdin to null, so the prompt reads EOF immediately.
    let output = Command::new(hotcell_bin())
        .current_dir(cwd.path())
        .args(["destroy", "--cell", cell_dir.path().to_str().unwrap()])
        .output()
        .expect("destroy hotcell");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "destroy at EOF should fail");
    assert!(stderr.contains("aborted"), "stderr: {stderr}");
    assert!(
        cell_dir.path().join(".cell/default").exists(),
        "cell state should survive an unanswered prompt"
    );
}

/// `--force` skips the prompt entirely: destruction proceeds even with no
/// stdin to answer it.
#[test]
fn destroy_force_bypasses_prompt() {
    let cwd = tempfile::tempdir().unwrap();
    let cell_dir = tempfile::tempdir().unwrap();
    fs::write(cwd.path().join("Cellfile"), "workdir = /work\n").unwrap();
    provision_cell(cwd.path(), cell_dir.path());

    let output = Command::new(hotcell_bin())
        .current_dir(cwd.path())
        .args([
            "destroy",
            "--force",
            "--cell",
            cell_dir.path().to_str().unwrap(),
        ])
        .output()
        .expect("destroy hotcell");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "destroy --force failed:\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(stdout.contains("destroyed cell"), "stdout: {stdout}");
    assert!(
        !stderr.contains("[y/N]"),
        "--force should not prompt, stderr: {stderr}"
    );
    assert!(!cell_dir.path().join(".cell/default").exists());
}

/// A blank answer (just Enter) is a decline.
#[test]
fn destroy_declines_on_empty_answer() {
    let cwd = tempfile::tempdir().unwrap();
    let cell_dir = tempfile::tempdir().unwrap();
    fs::write(cwd.path().join("Cellfile"), "workdir = /work\n").unwrap();
    provision_cell(cwd.path(), cell_dir.path());

    let output = destroy_with_stdin(cwd.path(), cell_dir.path(), &[], "\n");

    assert!(
        !output.status.success(),
        "destroy after blank answer should fail"
    );
    assert!(cell_dir.path().join(".cell/default").exists());
}
