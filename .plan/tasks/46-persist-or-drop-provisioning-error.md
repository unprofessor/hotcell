---
id: persist-or-drop-provisioning-error
aliases: [persist-or-drop-provisioning-error]
kind: task
parent: reconcile-spec-code-divergences
title: Persist or drop the provisioning_error message
status: todo
assignee: null
created: 2026-08-24
updated: 2026-08-24
tags: [spec, allium, provisioning, state]
depends_on: []
---

## Goal

Make the spec and code agree on whether a failed cell retains its provisioning
error message.

## Context

The spec retains a `provisioning_error: String` on the cell when status is
`provisioning_failed` (`hotcell.allium:159`), set from the error on failure
(line 330) and cleared on success/reset (261, 357). The code records failure as
a **bare boolean marker**: `state.rs` writes an empty `failed` file
(`FAILED_MARKER`, `mark_failed`, `src/state.rs:33,91,139`) and the error string
from provisioning (`src/provisioning.rs:161`) is surfaced to the caller but not
persisted into cell state. So a later inspection of a failed cell cannot recover
*why* it failed — the spec promises a message the state does not keep.

## Approach

Decide which side is authoritative:

1. **Persist the message (code follows spec).** Write the error text alongside
   (or into) the failed marker so a failed cell retains `provisioning_error`,
   and expose it wherever cell status is read. This is the more useful outcome
   — the message is already in hand at failure time; it is only being dropped.
   Keep the marker-file model (status derives from file presence) but let the
   file carry the message.
2. **Drop from spec (spec follows code).** If retaining the message is not
   wanted, change the spec so `provisioning_failed` carries no
   `provisioning_error` and the runtime just reports the error transiently.

Prefer option 1: retaining the failure reason is low-cost and clearly better
for debugging self-host runs. Confirm the file-presence status model stays
intact.

## Acceptance

- [ ] The decision (persist vs. drop) is recorded in Notes with rationale.
- [ ] If persisting: a failed cell retains its provisioning error message in
  state, readable after the run; the file-presence status model is preserved;
  a test covers write-and-read-back of the message. Spec unchanged (already
  matches) or tightened to describe where the message lives.
- [ ] If dropping: `provisioning_error` removed from the spec's cell state and
  transitions; `allium check` reports 0 errors.
- [ ] `cargo fmt --check`, `cargo clippy`, `cargo test` clean (code path); or
  `cargo test` green and `allium check` clean (spec-only path).

## Notes

- 2026-08-24 created from the `weed` pre-existing findings in
  [[realign-allium-spec-with-env-sources]]. Likely resolution: persist the
  message (option 1) — it is already produced at failure time and only being
  discarded, and keeping it aids debugging.
