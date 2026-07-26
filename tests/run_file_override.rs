//! Integration test: `--file|-f <CELLFILE>` overrides the conventional
//! `$PWD/Cellfile` path. The cell's state directory lives alongside the
//! given file (its parent), so a cell run from any directory can target a
//! Cellfile elsewhere without `cd`-ing into it.

use std::fs;
use std::process::Command;

/// Path to the built hotcell binary.
fn hotcell_bin() -> String {
    env!("CARGO_BIN_EXE_hotcell").to_string()
}

/// `--file` reads a Cellfile at an explicit path instead of `$PWD/Cellfile`,
/// and the cell runs successfully against it.
#[test]
fn file_flag_reads_cellfile_at_explicit_path() {
    let dir = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();

    // A Cellfile in `elsewhere`, not in `dir` (our cwd). There is no
    // $PWD/Cellfile, so success here proves `--file` was used.
    fs::write(elsewhere.path().join("Cellfile"), "workdir = /work\n").unwrap();

    let output = Command::new(hotcell_bin())
        .current_dir(dir.path())
        .args([
            "run",
            "--file",
            elsewhere.path().join("Cellfile").to_str().unwrap(),
            "--",
            "/bin/pwd",
        ])
        .output()
        .expect("run hotcell");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "hotcell run --file failed: exit {:?}\nstdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );
    assert_eq!(stdout.trim_end(), "/work", "stdout: {stdout}\nstderr: {stderr}");
}

/// `-f` is the short form of `--file`.
#[test]
fn short_flag_f_reads_cellfile_at_explicit_path() {
    let dir = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();

    fs::write(elsewhere.path().join("Cellfile"), "workdir = /work\n").unwrap();

    let output = Command::new(hotcell_bin())
        .current_dir(dir.path())
        .args([
            "run",
            "-f",
            elsewhere.path().join("Cellfile").to_str().unwrap(),
            "--",
            "/bin/pwd",
        ])
        .output()
        .expect("run hotcell");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "hotcell run -f failed: exit {:?}\nstdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );
    assert_eq!(stdout.trim_end(), "/work", "stdout: {stdout}\nstderr: {stderr}");
}

/// `--file` pointing at a non-existent file is a clear error, not a panic.
#[test]
fn file_flag_missing_file_errors() {
    let dir = tempfile::tempdir().unwrap();

    let output = Command::new(hotcell_bin())
        .current_dir(dir.path())
        .args(["run", "--file", "/no/such/Cellfile", "--", "/bin/pwd"])
        .output()
        .expect("run hotcell");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "expected failure");
    assert!(
        stderr.contains("no Cellfile found"),
        "expected 'no Cellfile found' in stderr, got: {stderr}"
    );
}

/// Without `--file`, a missing `$PWD/Cellfile` is still an error.
#[test]
fn no_file_flag_falls_back_to_cwd_cellfile() {
    let dir = tempfile::tempdir().unwrap();

    let output = Command::new(hotcell_bin())
        .current_dir(dir.path())
        .args(["run", "--", "/bin/pwd"])
        .output()
        .expect("run hotcell");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "expected failure");
    assert!(
        stderr.contains("no Cellfile found"),
        "expected 'no Cellfile found' in stderr, got: {stderr}"
    );
}
