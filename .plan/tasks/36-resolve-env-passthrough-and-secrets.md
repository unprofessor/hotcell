---
id: resolve-env-passthrough-and-secrets
aliases: [resolve-env-passthrough-and-secrets]
kind: task
parent: cellfile-kdl-grammar-and-secrets
title: Resolve passthrough and secret env vars at launch
status: review
assignee: null
created: 2026-08-23
updated: 2026-08-24
tags: [cellfile, secrets, plan]
depends_on: [replace-cellfile-parser-with-kdl]
---

## Goal

Resolve `Passthrough` and `Secret` env vars from hotcell's own environment
when the sandbox launches, and fail loudly before anything starts if a
declared var is missing.

## Context

The declaration holds only references. The resolved values must reach the
agent's `--setenv` arguments without ever entering `CellDeclaration`, so they
are never written to `provisioned_as.json` and never become part of cell
identity.

Resolution belongs where the plan's agent step is built (`plan.rs:196` builds
the env list today). A missing var must fail there — before the plan is
displayed, before the confirmation prompt, and before any state is touched —
so the failure is a config error, not a mid-run surprise.

## Acceptance

- [ ] `Passthrough` and `Secret` vars resolve from hotcell's process
  environment at plan-build time and reach the agent as `--setenv`.
- [ ] A declared var absent from the environment is a hard error naming the
  variable and the declaring node, for both `env NAME` and `secret NAME`. No
  silent omission.
- [ ] The error occurs before the plan is displayed and before any directory,
  marker or process is created — `--dry-run` fails the same way.
- [ ] Resolved values never appear in `CellDeclaration`, and therefore never
  in `provisioned_as.json`; a test asserts a secret's value is absent from the
  written state file.
- [ ] Changing a secret's value on the host does not make the cell stale and
  does not trigger re-provisioning.
- [ ] `cargo test` and `cargo clippy --all-targets` pass.

## Notes

- 2026-08-23 created. Hard-error-on-missing was chosen deliberately over
  silent omission: an unset provider key currently surfaces inside the cell as
  an opaque "Connection error", a failure mode already documented in
  `examples/pi-bootstrap/Cellfile`.

- 2026-08-24 the example `examples/pi-bootstrap/Cellfile` still tells users to
  write the provider API key as a literal `env GOOGLE_API_KEY="..."`. Now that
  `secret NAME` resolves from the host environment, that example could switch
  to `secret ANTHROPIC_API_KEY` (etc.) and drop the leak warning. Left
  untouched here to avoid scope creep — belongs with
  `document-kdl-cellfile-format`.

## Validation

Commands run in the worktree
(`.plan/worktrees/wt-resolve-env-passthrough-and-secrets`):

- `cargo build` — clean, `Finished dev profile`.
- `cargo clippy --all-targets` — clean, no warnings.
- `cargo test` — all suites pass, 0 failed across the workspace.

Per acceptance criterion:

- **Passthrough and secret resolve from hotcell's env, reach `--setenv`.**
  Implemented in `src/plan.rs::resolve_environment`, called from
  `build_execution_plan` with `|key| std::env::var(key).ok()`. Verified by unit
  test `plan::tests::passthrough_and_secret_resolve_from_the_host_environment`
  and integration test
  `run_cellfile_grammar::passthrough_and_secret_vars_resolve_from_the_host_environment`
  (asserts `--setenv PI_PROVIDER anthropic` and
  `--setenv HOTCELL_TEST_SECRET sk-from-host-env` appear in the plan).

- **Missing declared var is a hard error naming the variable and the node.**
  The error text is `env `NAME`: ...` for a passthrough and
  `secret `NAME`: ...` for a secret. Verified by unit tests
  `a_missing_passthrough_is_a_hard_error_naming_the_node`,
  `a_missing_secret_is_a_hard_error_naming_the_node`,
  `resolution_stops_at_the_first_missing_var` (no silent omission), and
  integration tests `a_missing_passthrough_is_a_hard_error` /
  `a_missing_secret_is_a_hard_error`.

- **Error occurs before the plan is displayed and before any state is
  touched; `--dry-run` fails the same way.** Resolution happens inside
  `build_execution_plan`, which `run_cell` (`src/cli.rs`) calls *before*
  `plan.render()`, the dry-run exit, the confirmation prompt, and
  `begin_provisioning`. The two integration tests above run through
  `hotcell run --dry-run` and confirm the run fails with a non-zero exit and
  nothing is created.

- **Resolved values never enter `CellDeclaration` / `provisioned_as.json`.**
  By construction: `resolve_environment` returns a separate `Vec<(String,
  String)>` and never writes back to the declaration; the declaration carries
  only `EnvSource::Secret`/`Passthrough` (key, no value). Asserted end-to-end
  by `run_env_secrets::a_resolved_secret_value_is_absent_from_the_state_file`,
  which runs a real cell with a distinctive secret value and greps the written
  `.cell/default/provisioned_as.json` for its absence (and the key's presence).

- **Changing a secret's value does not make the cell stale / re-provision.**
  Verified by `run_env_secrets::changing_a_secret_value_does_not_reprovision`:
  a shell-provisioned cell provisions on the first run, then a second run with
  a different `HOTCELL_TEST_SECRET` value reports `provisioning: skipped`.

- **`cargo test` and `cargo clippy --all-targets` pass.** Both clean (see
  above).

## Review

verdict: approved
reviewer: reviewer-agent
date: 2026-08-24

Re-verified independently in the worktree, not trusting the worker's notes.

- `cargo build`, `cargo clippy --all-targets` (forced rebuild via `touch
  src/plan.rs`), and `cargo test` all clean: 26 unit + integration suites, 0
  failed. New tests present and green: `plan::tests::*` (4),
  `run_cellfile_grammar::{passthrough_and_secret_vars_resolve_from_the_host_environment,
  a_missing_passthrough_is_a_hard_error, a_missing_secret_is_a_hard_error}`,
  `run_env_secrets::{a_resolved_secret_value_is_absent_from_the_state_file,
  changing_a_secret_value_does_not_reprovision}`.

- Manual `--dry-run` with a controlled env (Cellfile `env MY_PASS` + `secret
  MY_SECRET`): with both set, plan emits `--setenv MY_PASS passval` and
  `--setenv MY_SECRET topsecret123`, exit 0. With `MY_SECRET` unset: hard
  error "secret `MY_SECRET`: ... not set in hotcell's environment", exit 1,
  and NO `.cell` directory created — failure precedes any state/dir. (env
  passthrough missing-case covered by the integration test above.)

- Manual real run (`-y`, shell provisioner): `provisioned_as.json` records
  only `{"key":"MY_SECRET","source":"Secret"}` — grep for the actual value
  `ABC-111` returns 0 matches. A second run with a different value
  (`XYZ-999`) reports `provisioning: skipped`, confirming the secret is not
  part of cell identity.

- Security property holds by construction: `resolve_environment` returns a
  separate `Vec<(String,String)>` for `--setenv` and never writes back to
  `CellDeclaration`; `EnvSource::Secret`/`Passthrough` carry no value.
  Ordering in `cli.rs::run_cell` confirmed: `load_cell` (read-only, creates
  nothing) → `build_execution_plan` (resolution here) → `render` → dry-run
  exit → confirm → `begin_provisioning` (first dir creation).

- Scope clean: only the task file, `src/plan.rs`, `tests/run_cellfile_grammar.rs`,
  and new `tests/run_env_secrets.rs` touched. No other `.plan/` files. Example
  Cellfile deliberately left for `document-kdl-cellfile-format`, as noted.
