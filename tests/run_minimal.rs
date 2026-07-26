//! Integration test: a minimal `hotcell run /bin/pwd` inside a freshly
//! provisioned cell.
//!
//! Covers the `DeveloperCLI` surface's `RelayOnly` guarantee for the happy
//! path: provisioning succeeds, the program runs inside the isolated
//! sandbox, stdio is relayed faithfully, and the exit code passes through.
//! The sandbox's bind-mounted workdir appears as `/work`, so `pwd` prints
//! `/work`.

use std::process::Command;
use std::fs;

use tempfile::TempDir;

/// Path to the built hotcell binary.
fn hotcell_bin() -> String {
    env!("CARGO_BIN_EXE_hotcell").to_string()
}

#[test]
fn run_pwd_in_cell_prints_workdir() {
    // A directory containing an empty Cellfile: the minimal provisioning
    // case (stub succeeds with the default declaration).
    let dir = TempDir::new().expect("create temp dir");
    fs::write(dir.path().join("Cellfile"), "").expect("write Cellfile");

    // Run from the temp directory so hotcell finds $PWD/Cellfile.
    let output = Command::new(hotcell_bin())
        .current_dir(dir.path())
        .args(["run", "/bin/pwd"])
        .output()
        .expect("run hotcell");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "hotcell run failed: exit {:?}\nstdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );
    assert_eq!(stdout.trim_end(), "/work", "stdout: {stdout}\nstderr: {stderr}");
}

#[test]
fn run_relays_nonzero_exit_code() {
    // `false` always exits 1; the exit code should pass through unchanged.
    let dir = TempDir::new().expect("create temp dir");
    fs::write(dir.path().join("Cellfile"), "").expect("write Cellfile");

    let output = Command::new(hotcell_bin())
        .current_dir(dir.path())
        .args(["run", "/bin/false"])
        .output()
        .expect("run hotcell");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected exit code 1, got {:?}",
        output.status.code()
    );
}
