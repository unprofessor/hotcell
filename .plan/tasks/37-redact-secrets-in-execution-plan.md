---
id: redact-secrets-in-execution-plan
aliases: [redact-secrets-in-execution-plan]
kind: task
parent: cellfile-kdl-grammar-and-secrets
title: Redact secrets in the rendered execution plan
status: review
assignee: null
created: 2026-08-23
updated: 2026-08-24
tags: [secrets, plan]
depends_on: [resolve-env-passthrough-and-secrets]
---

## Goal

Stop printing secret values to stderr. Render `secret`-declared vars as
`<redacted>` in the execution plan while keeping the plan honest about what
will be set.

## Context

`plan::render` currently emits every env var verbatim, so
`--setenv ANTHROPIC_API_KEY sk-ant-...` is printed on every run, including
`--dry-run`, before the confirmation prompt. Verified 2026-08-23.

This breaks one of the plan's stated properties: the block can no longer be
copy-pasted and run verbatim when a secret is present. That trade is
deliberate, but the plan must say so rather than let the developer discover it
by pasting a broken command. The existing bridge footer is the pattern to
follow.

## Acceptance

- [x] `Secret` vars render as `--setenv NAME <redacted>`; the line is present,
  so the developer can see the var is being passed.
- [x] `Literal` and `Passthrough` vars render their real values — passthrough
  is not secret, and hiding it would obscure what the agent receives.
- [x] When at least one secret is present, the rendered plan carries a footer
  noting that the block will not run verbatim as shown.
- [x] A test asserts no secret value appears anywhere in rendered plan output,
  for both `run` and `--dry-run`.
- [x] `cargo test` and `cargo clippy --all-targets` pass.

## Notes

- 2026-08-23 created. Redaction is driven by the declared `Secret` source, not
  by a heuristic over key names — a heuristic would miss `PI_TOKEN` and mangle
  vars the developer wanted to read.
- 2026-08-24 implemented. Design: the step `Argument`s keep the *real* secret
  value (so `collapse` still runs the correct command — the plan stays the
  single source of truth for execution); redaction is display-only, applied in
  `ExecutionPlan::render`. The plan carries a new `secret_keys: Vec<String>`
  field (declared `Secret` keys); `render_step`/`render_argument` mask the
  value of any `--setenv <secret-key>` as the unquoted marker `<redacted>`, and
  `render` appends the footer when `secret_keys` is non-empty. The placeholder
  is intentionally left unquoted so it reads as a marker, not a literal — the
  footer already states the block will not run verbatim.
- 2026-08-24 updated `passthrough_and_secret_vars_resolve_from_the_host_environment`
  in `tests/run_cellfile_grammar.rs`: the `resolve-env-passthrough-and-secrets`
  task asserted the raw secret value reached `--setenv`, which is exactly the
  behaviour this task reverses. Now it asserts redaction + absence of the raw
  value + the footer. Not scope creep — it is this task's behaviour change.
- 2026-08-24 adjacent finding (not actioned): the docs task
  `document-kdl-cellfile-format` should mention that a Cellfile with a `secret`
  cannot be copy-pasted and run verbatim from the plan output.

## Validation

Ran in the task worktree on 2026-08-24.

- `cargo build` — clean (`Finished dev profile`).
- `cargo clippy --all-targets` — clean, no warnings.
- `cargo test` — all suites green (unit lib incl. new plan tests: 34 passed;
  every integration binary passed; 0 failed across the run).

New tests (`src/plan.rs`):
- `secret_setenv_line_is_present_but_redacted` — `--setenv ANTHROPIC_API_KEY
  <redacted>` present; raw value absent.
- `literal_and_passthrough_values_are_shown_in_full` — `PATH` and `PI_PROVIDER`
  show real values.
- `footer_appears_only_when_a_secret_is_present` — footer present with a secret,
  absent (and no `<redacted>`) without one; the no-secret plan shows the full
  value verbatim.
- `build_execution_plan_redacts_the_declared_secret` — end-to-end through
  `build_execution_plan`: secret resolved from the host env, `secret_keys`
  populated, value redacted in `render()`, raw value absent, footer present.
  `render()` is the single string printed on both `run` and `--dry-run` (the
  `eprintln!(plan.render())` in `cli.rs` runs before the `--dry-run` exit).

Updated test (`tests/run_cellfile_grammar.rs`, real `hotcell run --dry-run`):
- `passthrough_and_secret_vars_resolve_from_the_host_environment` — passthrough
  shows real value; secret shows `<redacted>`; raw `sk-from-host-env` absent;
  footer present.
- `a_plan_without_secrets_shows_every_value_and_no_footer` (new) — full value,
  no `<redacted>`, no footer.

Manual `--dry-run` (real binary):
- With `secret ANTHROPIC_API_KEY=sk-ant-REALVALUE-123`: rendered
  `--setenv ANTHROPIC_API_KEY <redacted>`, `--setenv PATH /opt/bin:/usr/bin`,
  `--setenv PI_PROVIDER anthropic`, footer printed, raw value never printed
  (grep for the value returned nothing), exit 0.
- Without any secret: full passthrough value shown, no `<redacted>`, no footer.
