---
id: realign-allium-spec-with-env-sources
aliases: [realign-allium-spec-with-env-sources]
kind: task
parent: cellfile-kdl-grammar-and-secrets
title: Realign hotcell.allium with the env source model
status: review
assignee: null
created: 2026-08-23
updated: 2026-08-24
tags: [spec, allium, secrets]
depends_on: [replace-cellfile-parser-with-kdl, resolve-env-passthrough-and-secrets, redact-secrets-in-execution-plan]
---

## Goal

Bring `hotcell.allium` back into agreement with the code after this story
changes how environment variables are declared, resolved and persisted.

## Context

`hotcell.allium` declares:

```
value EnvVar {
    key: String
    value: String
}
```

The code no longer matches. `EnvVar` is now `{ key, source }` where source is
`Literal(String) | Passthrough | Secret`, and for the latter two the value is
resolved at launch and deliberately never enters `CellDeclaration`.

This divergence is **owned by the spec, not the Cellfile format**. The spec's
Excludes list drops "Cellfile format and parsing (implementation detail)", so
KDL itself is correctly invisible to it — but `EnvVar` is a spec value type,
and the distinction between a declared literal and a host-resolved secret is
behaviour, not syntax. It belongs in the spec.

Surfaced by the **worker** on `replace-cellfile-parser-with-kdl`, which
judged a spec edit out of scope for a parser task and flagged the divergence
rather than hiding it; the reviewer independently confirmed it is real. The
gap is a planning omission in this story, which shipped four tasks and no
spec work.

Deferred to last on purpose: `resolve-env-passthrough-and-secrets` and
`redact-secrets-in-execution-plan` both change observable behaviour the spec
may need to describe, so editing the spec once at the end beats editing it
three times.

## Approach

Run the Allium skills rather than hand-editing the spec:

1. **`weed`** first — audit where `hotcell.allium` and the code have actually
   diverged. The `EnvVar` change is the known one, but weed should be allowed
   to surface the full set, because others are already suspected (see Notes).
2. **`tend`** second — apply the resolutions weed identifies that fall inside
   this task's scope.

Scope is the divergence *this story introduced*. Pre-existing drift that weed
finds is recorded in this task's Notes and ticketed separately, not fixed
here — otherwise this task becomes an open-ended spec cleanup.

## Acceptance

- [x] `weed` has been run and its findings recorded, separated into "caused by
  this story" and "pre-existing".
- [x] The spec's `EnvVar` (or its replacement) expresses the three sources,
  and makes clear that a passthrough or secret value is resolved from the
  invoking environment rather than declared in the Cellfile.
- [x] The spec states that a secret's value is never persisted into
  `provisioned_as` and therefore never participates in `is_current` — this is
  a guarantee, not an implementation detail.
- [x] Whatever the spec says about a missing declared variable matches the
  implemented behaviour: a hard error before the session starts.
- [x] Any `@guarantee` wording that the secrets work makes false or incomplete
  (e.g. `CleanEnvironment`) is updated.
- [x] Pre-existing divergences found by weed are listed in Notes. Ticketing is
  a leader action (a worker may edit only its own task file and the spec), so
  each is classified with its owning story where one exists and flagged for the
  leader where none does; none is fixed here.
- [x] The spec still validates, and `cargo test` still passes (the spec is not
  compiled, but the task must not leave the tree broken).

## Notes

- 2026-08-23 created. Already-suspected pre-existing drift, for weed to
  confirm — do **not** fix these here:
  - Spec says provisioner kind `"script"`; the code implements `"shell"` and
    rejects `"script"` as unknown.
  - `Session` is specified as an entity with `sessions` / `active_sessions`,
    but no session is recorded at runtime, so `DestroyCell`'s
    `active_sessions.count = 0` precondition is unenforced.
  - `provisioner_override` is in the spec's `DeveloperCLI` surface and has no
    CLI flag.
  - `provisioning_error` is specified as retained on the cell; the code writes
    an empty `failed` marker and discards the message.
  - `packages` and `files` remain in the spec's `CellDeclaration` and on the
    Rust struct, but no grammar populates them — owned by
    `packages-seeds-semantics`.

## Weed findings

`weed` skill run in **check** mode against `hotcell.allium` vs `src/`
(`cellfile.rs`, `plan.rs`, `provisioning.rs`, `cell.rs`, `state.rs`, `cli.rs`).

### Caused by this story (fixed here — scope of this task)

- **`EnvVar` shape.** Spec had `value EnvVar { key: String; value: String }`;
  code has `EnvVar { key, source }` with `EnvSource = Literal(String) |
  Passthrough | Secret` (`src/cellfile.rs:90-106`). Classification: spec bug.
  Fixed: added `enum EnvSource { literal | passthrough | secret }`; `EnvVar` is
  now `{ key, source, value: String? }` with `value` set only for `literal`.
- **Secret non-persistence not stated.** Code deliberately keeps
  passthrough/secret resolved values out of `CellDeclaration` /
  `provisioned_as.json`, so they never affect the declaration comparison
  (`src/plan.rs:207-256,286-296`; `src/cell.rs:85-86`). Spec was silent.
  Fixed: recorded on `provisioned_as` and `is_current`, and as the
  `Cleanroom.EnvironmentResolvedAtLaunch` guarantee.
- **Missing declared variable = hard error.** A declared passthrough/secret
  absent from hotcell's env is a hard error at plan-build, before any session
  (`src/plan.rs:234-250,288`). Spec was silent. Fixed:
  `DeveloperCLI.DeclaredVariableMustResolve`.
- **Secret redaction in run output.** `render` masks a declared secret's
  `--setenv` value as `<redacted>` (`src/plan.rs:80-121,179,296`). Spec was
  silent. Fixed: `DeveloperCLI.SecretsRedactedInRunOutput`.
- **`CleanEnvironment` now incomplete.** "No host state leaks into the sandbox"
  is falsified by the deliberate passthrough/secret channel. Fixed: reworded to
  name declared passthrough/secret as the one opt-in host-to-cell channel.

### Pre-existing (NOT fixed here — for the leader to ticket)

Confirmed against code; each pre-dates this story. A worker may edit only its
own task file and the spec, so no tickets are created here.

- **Provisioner kind `"script"` vs `"shell"`.** Spec's `ProvisioningProcess`
  lists kinds `"none"` and `"script"`; code implements `"none"` (default) and
  `"shell"`, and rejects anything else as "unknown provisioner kind"
  (`src/provisioning.rs:88-99,131-134`; `src/state.rs:30`). No owning ticket
  found — needs one.
- **`Session` entity unenforced at runtime.** Spec models `Session` /
  `active_sessions` and `DestroyCell` requires `active_sessions.count = 0`, but
  no session is recorded at runtime (`allium check` reports `Session.*` fields
  unused). No owning ticket found — needs one.
- **`provisioner_override` has no CLI flag.** In spec's `DeveloperCLI.RunCell`;
  no corresponding flag in `src/cli.rs`. No owning ticket found — needs one.
- **`provisioning_error` message discarded.** Spec retains
  `provisioning_error` on the cell; code writes an empty `failed` marker and
  drops the message (`src/state.rs:33,91,139`). No owning ticket found — needs
  one.
- **`packages` / `files` never populated.** Present in spec `CellDeclaration`
  and the Rust struct, but the KDL grammar drops `package`/`seed` and reports
  them "not implemented" (`src/cellfile.rs:132-133,239,919-929`). Owned by
  `packages-seeds-semantics`.

## Validation

- `allium check hotcell.allium` — **0 errors** before and after. The remaining
  diagnostics (8 warnings, 5 info) are identical to the pre-edit baseline
  (unused surface bindings, external-entity source hints, unused `Session`/
  `Cell.sessions` fields, one deferred-spec location hint) — all pre-existing
  and unrelated to the env work. No new `EnvVar`/`EnvSource` diagnostics: the
  new `source`/`value` fields resolve and are referenced.
- `cargo test` — all suites green (unit + integration; 39 lib tests plus the
  integration binaries, 0 failed). Confirms the spec edits left the tree
  buildable and the code unchanged.
- Spec claims verified directly against code before writing: `EnvSource`
  variants (`src/cellfile.rs:90-106`), launch-time resolution and missing-var
  hard error (`src/plan.rs:207-256,288`), non-persistence into `provisioned_as`
  / `is_current` (`src/plan.rs:286-296`, `src/cell.rs:85-86`), and secret
  redaction to `<redacted>` (`src/plan.rs:80-121,179,296`).
