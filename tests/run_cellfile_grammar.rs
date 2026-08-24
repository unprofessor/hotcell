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
    plan_with_cellfile_env(content, &[], &[])
}

/// As [`plan_with_cellfile`], but with control over hotcell's own environment:
/// `set` names vars to set (and their values), `remove` names vars to unset,
/// so a test can decide exactly which passthrough/secret keys resolve.
fn plan_with_cellfile_env(
    content: &str,
    set: &[(&str, &str)],
    remove: &[&str],
) -> (bool, String) {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("Cellfile"), content).expect("write Cellfile");
    let mut command = Command::new(hotcell_bin());
    command
        .current_dir(dir.path())
        .args(["run", "--dry-run", "--", "/bin/true"]);
    for (key, value) in set {
        command.env(key, value);
    }
    for key in remove {
        command.env_remove(key);
    }
    let output = command.output().expect("run hotcell");
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

/// A passthrough (`env NAME`) and a secret (`secret NAME`) resolve from
/// hotcell's own environment and reach the agent as `--setenv`.
#[test]
fn passthrough_and_secret_vars_resolve_from_the_host_environment() {
    let (success, stderr) = plan_with_cellfile_env(
        "workdir \"/work\"\nenv PI_PROVIDER\nsecret HOTCELL_TEST_SECRET\n",
        &[
            ("PI_PROVIDER", "anthropic"),
            ("HOTCELL_TEST_SECRET", "sk-from-host-env"),
        ],
        &[],
    );
    assert!(success, "stderr: {stderr}");
    assert!(
        stderr.contains("--setenv PI_PROVIDER anthropic"),
        "passthrough must reach --setenv: {stderr}"
    );
    assert!(
        stderr.contains("--setenv HOTCELL_TEST_SECRET sk-from-host-env"),
        "secret must reach --setenv: {stderr}"
    );
}

/// A declared passthrough absent from hotcell's environment is a hard error
/// naming the variable and its `env` node — no silent omission — and it fails
/// under `--dry-run`, before the plan is displayed or any state is touched.
#[test]
fn a_missing_passthrough_is_a_hard_error() {
    let (success, stderr) =
        plan_with_cellfile_env("workdir \"/work\"\nenv PI_PROVIDER\n", &[], &["PI_PROVIDER"]);
    assert!(!success, "a missing passthrough must fail the run: {stderr}");
    assert!(
        stderr.contains("env `PI_PROVIDER`") && stderr.contains("not set in hotcell"),
        "stderr: {stderr}"
    );
}

/// A declared secret absent from hotcell's environment is a hard error naming
/// the variable and its `secret` node, and fails under `--dry-run` too.
#[test]
fn a_missing_secret_is_a_hard_error() {
    let (success, stderr) = plan_with_cellfile_env(
        "workdir \"/work\"\nsecret ANTHROPIC_API_KEY\n",
        &[],
        &["ANTHROPIC_API_KEY"],
    );
    assert!(!success, "a missing secret must fail the run: {stderr}");
    assert!(
        stderr.contains("secret `ANTHROPIC_API_KEY`") && stderr.contains("not set in hotcell"),
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

/// A repeated single-valued node (here `provision`) fails the run through
/// `--dry-run` with a located error citing both occurrences, and creates no
/// `.cell` state — the failure is raised while reading the Cellfile, before
/// any state exists. This is the ticket's bug: last-wins used to drop the
/// first block's `host-path "/etc/hostname"` silently at exit 0.
#[test]
fn a_duplicate_single_valued_node_fails_and_creates_no_state() {
    let dir = tempfile::tempdir().unwrap();
    let text = "\
provision \"shell\" script=\"./p.sh\" { host-path \"/etc/hostname\" }
provision \"shell\" script=\"./p.sh\" { host-path \"/etc/machine-id\" }
";
    fs::write(dir.path().join("Cellfile"), text).expect("write Cellfile");
    let output = Command::new(hotcell_bin())
        .current_dir(dir.path())
        .args(["run", "--dry-run", "--", "/bin/true"])
        .output()
        .expect("run hotcell");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "must not run: {stderr}");
    assert!(
        stderr.contains("`provision` may be declared only once"),
        "stderr: {stderr}"
    );
    assert!(stderr.contains("Cellfile:2:1:"), "stderr: {stderr}");
    assert!(
        stderr.contains("first declared at") && stderr.contains("Cellfile:1:1"),
        "stderr: {stderr}"
    );
    assert!(
        !dir.path().join(".cell").exists(),
        "no cell state may be created when parsing fails"
    );
}
