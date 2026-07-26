//! Integration test: the root-level `--file` and `--cell` flags apply to
//! `destroy` and `status` as well as `run`. `--cell` relocates the state
//! directory those commands read from and act on; `--file` selects the
//! Cellfile (and thus the cell identity) they target.

use std::fs;
use std::process::Command;

/// Path to the built hotcell binary.
fn hotcell_bin() -> String {
    env!("CARGO_BIN_EXE_hotcell").to_string()
}

/// `destroy` honors `--cell`: a cell provisioned with `--cell DIR` is
/// destroyed via `destroy --cell DIR`, and the state directory is removed.
#[test]
fn destroy_honors_cell_flag() {
    let cwd = tempfile::tempdir().unwrap();
    let cell_dir = tempfile::tempdir().unwrap();

    fs::write(cwd.path().join("Cellfile"), "workdir = /work\n").unwrap();

    // Provision a cell with state rooted in cell_dir.
    let run = Command::new(hotcell_bin())
        .current_dir(cwd.path())
        .args([
            "run",
            "--cell",
            cell_dir.path().to_str().unwrap(),
            "--",
            "/bin/pwd",
        ])
        .output()
        .expect("run hotcell");
    assert!(run.status.success(), "provisioning run failed");
    assert!(cell_dir.path().join(".cell").exists());

    // Destroy it via --cell (without --cell, destroy would look in cwd and
    // find no state).
    let destroy = Command::new(hotcell_bin())
        .current_dir(cwd.path())
        .args(["destroy", "--cell", cell_dir.path().to_str().unwrap()])
        .output()
        .expect("destroy hotcell");

    let stdout = String::from_utf8_lossy(&destroy.stdout);
    let stderr = String::from_utf8_lossy(&destroy.stderr);
    assert!(
        destroy.status.success(),
        "destroy --cell failed: exit {:?}\nstdout: {stdout}\nstderr: {stderr}",
        destroy.status.code()
    );
    assert!(stdout.contains("destroyed cell"), "stdout: {stdout}");
    assert!(
        !cell_dir.path().join(".cell/default").exists(),
        "cell state should be gone after destroy, stderr: {stderr}"
    );
}

/// `status` honors `--cell`: it lists cells rooted under the given directory
/// rather than `$PWD`.
#[test]
fn status_honors_cell_flag() {
    let cwd = tempfile::tempdir().unwrap();
    let cell_dir = tempfile::tempdir().unwrap();

    fs::write(cwd.path().join("Cellfile"), "workdir = /work\n").unwrap();

    // Provision a cell with state in cell_dir, plus a second named cell.
    Command::new(hotcell_bin())
        .current_dir(cwd.path())
        .args([
            "run",
            "--cell",
            cell_dir.path().to_str().unwrap(),
            "--",
            "/bin/pwd",
        ])
        .output()
        .expect("run hotcell");
    Command::new(hotcell_bin())
        .current_dir(cwd.path())
        .args([
            "run",
            "--name",
            "alt",
            "--cell",
            cell_dir.path().to_str().unwrap(),
            "--",
            "/bin/pwd",
        ])
        .output()
        .expect("run hotcell alt");

    let status = Command::new(hotcell_bin())
        .current_dir(cwd.path())
        .args(["status", "--cell", cell_dir.path().to_str().unwrap()])
        .output()
        .expect("status hotcell");

    let stdout = String::from_utf8_lossy(&status.stdout);
    let stderr = String::from_utf8_lossy(&status.stderr);
    assert!(
        status.status.success(),
        "status --cell failed: stderr: {stderr}"
    );
    assert!(
        stdout.contains("default"),
        "expected 'default' in status: {stdout}"
    );
    assert!(stdout.contains("alt"), "expected 'alt' in status: {stdout}");

    // And status without --cell (looking in cwd) should report no cells,
    // proving --cell was actually used.
    let bare = Command::new(hotcell_bin())
        .current_dir(cwd.path())
        .args(["status"])
        .output()
        .expect("status hotcell");
    let bare_out = String::from_utf8_lossy(&bare.stdout);
    assert!(
        bare_out.contains("no cells"),
        "status without --cell should find no cells, got: {bare_out}"
    );
}

/// `status` honors `--file`: it reads the Cellfile from the given path so the
/// current/stale comparison reflects it.
#[test]
fn status_honors_file_flag() {
    let cwd = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();

    // No Cellfile in cwd; the only one is in elsewhere.
    fs::write(elsewhere.path().join("Cellfile"), "workdir = /work\n").unwrap();

    // status --file should succeed (it reads the Cellfile from elsewhere)
    // and report no cells, since none have been provisioned there.
    let status = Command::new(hotcell_bin())
        .current_dir(cwd.path())
        .args([
            "status",
            "--file",
            elsewhere.path().join("Cellfile").to_str().unwrap(),
        ])
        .output()
        .expect("status hotcell");

    let stdout = String::from_utf8_lossy(&status.stdout);
    let stderr = String::from_utf8_lossy(&status.stderr);
    assert!(
        status.status.success(),
        "status --file failed: stderr: {stderr}"
    );
    assert!(
        stdout.contains("no cells"),
        "expected 'no cells', got: {stdout}"
    );

    // status without --file should fail: no Cellfile in cwd.
    let bare = Command::new(hotcell_bin())
        .current_dir(cwd.path())
        .args(["status"])
        .output()
        .expect("status hotcell");
    assert!(!bare.status.success(), "expected failure without --file");
}

/// `destroy` with no cell to destroy is an error, and `--cell` does not
/// change that — it just picks where to look.
#[test]
fn destroy_cell_flag_missing_cell_errors() {
    let cwd = tempfile::tempdir().unwrap();
    let cell_dir = tempfile::tempdir().unwrap();

    fs::write(cwd.path().join("Cellfile"), "workdir = /work\n").unwrap();

    let output = Command::new(hotcell_bin())
        .current_dir(cwd.path())
        .args(["destroy", "--cell", cell_dir.path().to_str().unwrap()])
        .output()
        .expect("destroy hotcell");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "expected failure");
    assert!(
        stderr.contains("not in a destroyable state") || stderr.contains("destroyable"),
        "expected destroyable-state error, got: {stderr}"
    );
}
