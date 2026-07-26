//! Integration test: `-c|--cell <DIR>` overrides the cell directory — the
//! root for cell state (`.cell/...`) — independently of where the Cellfile
//! was read from. By default state lives alongside the Cellfile; `--cell`
//! decouples the two.

use std::fs;
use std::process::Command;

/// Path to the built hotcell binary.
fn hotcell_bin() -> String {
    env!("CARGO_BIN_EXE_hotcell").to_string()
}

/// `--cell <DIR>` puts cell state under `<DIR>/.cell`, not under the
/// Cellfile's parent. The cell still runs successfully.
#[test]
fn cell_flag_relocates_state_directory() {
    let cwd = tempfile::tempdir().unwrap();
    let cell_dir = tempfile::tempdir().unwrap();

    // A Cellfile in cwd (the conventional location).
    fs::write(cwd.path().join("Cellfile"), "workdir = /work\n").unwrap();

    let output = Command::new(hotcell_bin())
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

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "hotcell run --cell failed: exit {:?}\nstdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );
    assert_eq!(stdout.trim_end(), "/work");

    // State must live under the overridden cell dir, not under cwd.
    assert!(
        cell_dir.path().join(".cell").exists(),
        "expected .cell under --cell dir, stderr: {stderr}"
    );
    assert!(
        !cwd.path().join(".cell").exists(),
        "state leaked into the Cellfile's parent"
    );
}

/// `-c` is the short form of `--cell`.
#[test]
fn short_flag_c_relocates_state_directory() {
    let cwd = tempfile::tempdir().unwrap();
    let cell_dir = tempfile::tempdir().unwrap();

    fs::write(cwd.path().join("Cellfile"), "workdir = /work\n").unwrap();

    let output = Command::new(hotcell_bin())
        .current_dir(cwd.path())
        .args([
            "run",
            "-c",
            cell_dir.path().to_str().unwrap(),
            "--",
            "/bin/pwd",
        ])
        .output()
        .expect("run hotcell");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "hotcell run -c failed: stderr: {stderr}"
    );
    assert!(
        cell_dir.path().join(".cell").exists(),
        "expected .cell under -c dir, stderr: {stderr}"
    );
}

/// `--file` and `--cell` combine: the Cellfile is read from one place while
/// state is kept in another. Neither location's role bleeds into the other.
#[test]
fn file_and_cell_combine_independently() {
    let cwd = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let cell_dir = tempfile::tempdir().unwrap();

    // No Cellfile in cwd; the only Cellfile is in `elsewhere`.
    fs::write(elsewhere.path().join("Cellfile"), "workdir = /work\n").unwrap();

    let output = Command::new(hotcell_bin())
        .current_dir(cwd.path())
        .args([
            "run",
            "--file",
            elsewhere.path().join("Cellfile").to_str().unwrap(),
            "--cell",
            cell_dir.path().to_str().unwrap(),
            "--",
            "/bin/pwd",
        ])
        .output()
        .expect("run hotcell");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "hotcell run --file --cell failed: exit {:?}\nstdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );
    assert_eq!(stdout.trim_end(), "/work");

    // State in the --cell dir, not beside the Cellfile and not in cwd.
    assert!(
        cell_dir.path().join(".cell").exists(),
        "expected .cell under --cell dir, stderr: {stderr}"
    );
    assert!(
        !elsewhere.path().join(".cell").exists(),
        "state leaked beside the --file Cellfile"
    );
    assert!(
        !cwd.path().join(".cell").exists(),
        "state leaked into cwd"
    );
}
