---
id: reconcile-provisioner-kind-name
aliases: [reconcile-provisioner-kind-name]
kind: task
parent: reconcile-spec-code-divergences
title: Reconcile provisioner kind name (script vs shell)
status: todo
assignee: null
created: 2026-08-24
updated: 2026-08-24
tags: [spec, allium, provisioning]
depends_on: []
---

## Goal

Make the spec and code agree on the name of the non-`none` provisioner kind.

## Context

`hotcell.allium` (lines ~42, 83) names the two provisioner kinds `"none"` and
`"script"`, and describes `"script"` as running the script at `script`. The
code implements `"none"` (the default, `src/state.rs:30`
`DEFAULT_PROVISIONER_KIND`) and `"shell"`, parses `provision "shell"
script="..."` (`src/cellfile.rs:114,308-320`), and **rejects `"script"` as an
unknown provisioner kind** (`src/provisioning.rs:88-99,131-134`). So the exact
kind string the spec sanctions is the one the code refuses.

The code side is the shipped, documented, and tested behaviour — README and
`examples/pi-bootstrap/Cellfile` both use `provision "shell"`. Correcting the
spec to `"shell"` is almost certainly right; confirm before editing.

## Approach

1. Confirm current behaviour directly: which kind string the parser accepts,
   what the default is, and what error a `"script"` kind produces.
2. Update `hotcell.allium` so the non-`none` kind is `"shell"` throughout
   (the kinds list, the `ProvisioningProcess` prose, and any example), unless
   the decision is instead to rename the code's kind to `"script"` — in which
   case change the code, README, and example together and keep the spec.
3. Keep `kind` an open set on the code side if it already is (`cellfile.rs`
   treats `kind` as an open string); the spec need only fix its named example.

## Acceptance

- [ ] Spec and code use the same string for the shell/script provisioner kind;
  a reader following the spec would write a Cellfile the parser accepts.
- [ ] The decision (rename spec to `shell`, vs. rename code to `script`) is
  recorded in Notes with its rationale.
- [ ] `allium check hotcell.allium` reports 0 errors, and the diagnostic set is
  no worse than baseline.
- [ ] If code changed: `cargo fmt --check`, `cargo clippy`, `cargo test` clean,
  and README/example updated to match. If spec-only: `cargo test` still green.

## Notes

- 2026-08-24 created from the `weed` pre-existing findings in
  [[realign-allium-spec-with-env-sources]]. Expected resolution: fix the spec to
  `"shell"` (code is authoritative and user-facing), but verify first.
