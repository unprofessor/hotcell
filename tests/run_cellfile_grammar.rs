//! Integration tests for the KDL Cellfile grammar as the user meets it: the
//! errors a bad or outdated Cellfile produces on stderr, and the exit code.
//!
//! These run through `hotcell run --dry-run`, so nothing is provisioned or
//! sandboxed — the Cellfile is read and the plan is compiled, which is where
//! every failure below is raised.

use std::fs;
use std::process::Command;

/// Path to the built hotcell binary.
fn hotcell_bin() -> String {
    env!("CARGO_BIN_EXE_hotcell").to_string()
}

/// Write `content` as a Cellfile in a fresh temp dir, plan a run against it,
/// and return `(success, stderr)`.
fn plan_with_cellfile(content: &str) -> (bool, String) {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("Cellfile"), content).expect("write Cellfile");
    let output = Command::new(hotcell_bin())
        .current_dir(dir.path())
        .args(["run", "--dry-run", "--", "/bin/true"])
        .output()
        .expect("run hotcell");
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// A Cellfile still in the pre-KDL `key = value` format fails with a
/// migration hint, not a bare KDL syntax error: the reader is told which
/// line looks old and what the format is now.
#[test]
fn a_v1_cellfile_reports_the_format_change() {
    let (success, stderr) =
        plan_with_cellfile("provision.type = shell\nprovision.script = ./p.sh\nworkdir = /work\n");
    assert!(!success, "an old-format Cellfile must not run: {stderr}");
    assert!(
        stderr.contains("looks like the old `key = value` Cellfile format"),
        "stderr: {stderr}"
    );
    assert!(stderr.contains("Cellfile:1:"), "stderr: {stderr}");
}

/// The parser records passthrough and secret vars, but nothing resolves them
/// yet. Rather than hand the agent a cell silently missing a variable it was
/// promised, the run fails and says so.
#[test]
fn passthrough_and_secret_vars_are_not_yet_resolved() {
    let (success, stderr) = plan_with_cellfile("workdir \"/work\"\nenv PI_PROVIDER\n");
    assert!(!success, "stderr: {stderr}");
    assert!(
        stderr.contains("env `PI_PROVIDER`") && stderr.contains("not yet implemented"),
        "stderr: {stderr}"
    );

    let (success, stderr) = plan_with_cellfile("workdir \"/work\"\nsecret ANTHROPIC_API_KEY\n");
    assert!(!success, "stderr: {stderr}");
    assert!(
        stderr.contains("secret `ANTHROPIC_API_KEY`") && stderr.contains("not yet implemented"),
        "stderr: {stderr}"
    );
}

/// A literal `env` var still reaches the plan as a `--setenv` argument.
#[test]
fn literal_env_vars_reach_the_plan() {
    let (success, stderr) = plan_with_cellfile("workdir \"/work\"\nenv GREETING=\"hello there\"\n");
    assert!(success, "stderr: {stderr}");
    assert!(
        stderr.contains("--setenv GREETING 'hello there'"),
        "stderr: {stderr}"
    );
}

/// `package` and `seed` were dropped from the grammar rather than ported.
/// Naming either says so instead of reporting an unknown node.
#[test]
fn package_and_seed_report_that_they_are_gone() {
    let (success, stderr) = plan_with_cellfile("package \"node\"\n");
    assert!(!success, "stderr: {stderr}");
    assert!(
        stderr.contains("`package` is not implemented"),
        "stderr: {stderr}"
    );

    let (success, stderr) = plan_with_cellfile("seed \"./src\"\n");
    assert!(!success, "stderr: {stderr}");
    assert!(
        stderr.contains("`seed` is not implemented"),
        "stderr: {stderr}"
    );
}

/// The grammar fails closed: an unrecognised node is an error, located in
/// the file, not silently ignored.
#[test]
fn an_unknown_node_is_a_located_error() {
    let (success, stderr) = plan_with_cellfile("workdir \"/work\"\nnett {\n    allow \"h\"\n}\n");
    assert!(!success, "stderr: {stderr}");
    assert!(
        stderr.contains("unknown Cellfile node `nett`"),
        "stderr: {stderr}"
    );
    assert!(stderr.contains("Cellfile:2:1:"), "stderr: {stderr}");
}
