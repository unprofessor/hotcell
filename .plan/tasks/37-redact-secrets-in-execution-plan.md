---
id: redact-secrets-in-execution-plan
aliases: [redact-secrets-in-execution-plan]
kind: task
parent: cellfile-kdl-grammar-and-secrets
title: Redact secrets in the rendered execution plan
status: in_progress
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

- [ ] `Secret` vars render as `--setenv NAME <redacted>`; the line is present,
  so the developer can see the var is being passed.
- [ ] `Literal` and `Passthrough` vars render their real values — passthrough
  is not secret, and hiding it would obscure what the agent receives.
- [ ] When at least one secret is present, the rendered plan carries a footer
  noting that the block will not run verbatim as shown.
- [ ] A test asserts no secret value appears anywhere in rendered plan output,
  for both `run` and `--dry-run`.
- [ ] `cargo test` and `cargo clippy --all-targets` pass.

## Notes

- 2026-08-23 created. Redaction is driven by the declared `Secret` source, not
  by a heuristic over key names — a heuristic would miss `PI_TOKEN` and mangle
  vars the developer wanted to read.
