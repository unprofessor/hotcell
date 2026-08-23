---
id: resolve-env-passthrough-and-secrets
aliases: [resolve-env-passthrough-and-secrets]
kind: task
parent: cellfile-kdl-grammar-and-secrets
title: Resolve passthrough and secret env vars at launch
status: todo
assignee: null
created: 2026-08-23
updated: 2026-08-23
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
