//! Integration tests for resolved passthrough/secret env vars as they meet
//! durable state: a resolved secret value reaches the agent but never enters
//! `CellDeclaration`, so it is never written to `provisioned_as.json` and
//! never becomes part of cell identity — rotating it on the host does not
//! force a re-provision.

use std::fs;
use std::process::Command;

use tempfile::TempDir;

fn hotcell_bin() -> String {
    env!("CARGO_BIN_EXE_hotcell").to_string()
}

/// A run with `secret NAME` declared: the resolved value reaches the agent,
/// but the state file records only the declaration, never the value.
#[test]
fn a_resolved_secret_value_is_absent_from_the_state_file() {
    let secret_value = "sk-super-secret-value-do-not-persist";

    let dir = TempDir::new().expect("create temp dir");
    fs::write(
        dir.path().join("Cellfile"),
        "workdir \"/work\"\nsecret HOTCELL_TEST_SECRET\n",
    )
    .expect("write Cellfile");

    let output = Command::new(hotcell_bin())
        .current_dir(dir.path())
        .env("HOTCELL_TEST_SECRET", secret_value)
        .args(["run", "-y", "/bin/true"])
        .output()
        .expect("run hotcell");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "run failed: exit {:?}\nstderr: {stderr}",
        output.status.code()
    );

    let state = dir.path().join(".cell/default/provisioned_as.json");
    let recorded = fs::read_to_string(&state).expect("read provisioned_as.json");
    assert!(
        !recorded.contains(secret_value),
        "the secret value must never be persisted to state:\n{recorded}"
    );
    // The declaration is still recorded — only the key is, not the value.
    assert!(
        recorded.contains("HOTCELL_TEST_SECRET"),
        "the declaration should still name the secret key:\n{recorded}"
    );
}

/// Rotating a secret's value on the host does not make the cell stale: the
/// declaration is unchanged (it carries no value), so the second run skips
/// re-provisioning even though the secret differs.
#[test]
fn changing_a_secret_value_does_not_reprovision() {
    let dir = TempDir::new().expect("create temp dir");
    fs::write(
        dir.path().join("Cellfile"),
        "provision \"shell\" script=\"./provision.sh\"\nworkdir \"/work\"\n\
         secret HOTCELL_TEST_SECRET\n",
    )
    .expect("write Cellfile");
    fs::write(
        dir.path().join("provision.sh"),
        "#!/usr/bin/env bash\nset -euo pipefail\nmkdir -p \"$HOTCELL_WORKDIR_HOST\"\n",
    )
    .expect("write provision.sh");
    fs::set_permissions(
        dir.path().join("provision.sh"),
        std::os::unix::fs::PermissionsExt::from_mode(0o755),
    )
    .expect("chmod provision.sh");

    let run = |value: &str| {
        Command::new(hotcell_bin())
            .current_dir(dir.path())
            .env("HOTCELL_TEST_SECRET", value)
            .args(["run", "-y", "/bin/true"])
            .output()
            .expect("run hotcell")
    };

    // First run provisions from scratch with one secret value.
    let first = run("secret-value-one");
    let first_err = String::from_utf8_lossy(&first.stderr);
    assert!(
        first.status.success(),
        "first run failed: {:?}\nstderr: {first_err}",
        first.status.code()
    );
    assert!(
        !first_err.contains("provisioning: skipped"),
        "first run should provision, not skip: {first_err}"
    );

    // Second run with a *different* secret value: still current, so
    // provisioning is skipped — the changed secret did not invalidate the cell.
    let second = run("secret-value-two-different");
    let second_err = String::from_utf8_lossy(&second.stderr);
    assert!(
        second.status.success(),
        "second run failed: {:?}\nstderr: {second_err}",
        second.status.code()
    );
    assert!(
        second_err.contains("provisioning: skipped"),
        "changing the secret must not force a re-provision: {second_err}"
    );
}
