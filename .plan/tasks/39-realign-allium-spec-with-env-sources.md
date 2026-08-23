---
id: realign-allium-spec-with-env-sources
aliases: [realign-allium-spec-with-env-sources]
kind: task
parent: cellfile-kdl-grammar-and-secrets
title: Realign hotcell.allium with the env source model
status: todo
assignee: null
created: 2026-08-23
updated: 2026-08-23
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

Surfaced by the reviewer of `replace-cellfile-parser-with-kdl`, which
correctly judged a spec edit out of scope for a parser task. The gap is a
planning omission in this story, which shipped four tasks and no spec work.

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

- [ ] `weed` has been run and its findings recorded, separated into "caused by
  this story" and "pre-existing".
- [ ] The spec's `EnvVar` (or its replacement) expresses the three sources,
  and makes clear that a passthrough or secret value is resolved from the
  invoking environment rather than declared in the Cellfile.
- [ ] The spec states that a secret's value is never persisted into
  `provisioned_as` and therefore never participates in `is_current` — this is
  a guarantee, not an implementation detail.
- [ ] Whatever the spec says about a missing declared variable matches the
  implemented behaviour: a hard error before the session starts.
- [ ] Any `@guarantee` wording that the secrets work makes false or incomplete
  (e.g. `CleanEnvironment`) is updated.
- [ ] Pre-existing divergences found by weed are listed in Notes with a
  ticket raised for each, or explicitly waived.
- [ ] The spec still validates, and `cargo test` still passes (the spec is not
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
