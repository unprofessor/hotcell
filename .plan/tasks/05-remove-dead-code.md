---
id: remove-dead-code
kind: task
parent: polish-and-docs
title: Remove dead-code allow and decide on session wiring
status: done
assignee: null
created: 2026-07-29
updated: 2026-08-12
tags: []
depends_on: []
---

## Goal

Remove dead-code allow and decide on session wiring

## Context

Parent: `polish-and-docs`. Files: `src/main.rs` (`#![allow(dead_code)]`),
`src/session.rs` (unused `Session`/`SessionStatus`), `src/cli.rs` (does
session-like work inline without persisting `Session` records).

## Acceptance

- [ ] Decide: wire durable session records (persist `Session` to state dir,
      surface in `status`) OR drop `session.rs` and its `mod` declaration.
- [ ] Remove `#![allow(dead_code)]` if no longer needed; if kept, scope it to
      the specific items with a clear reason.
- [ ] `cargo build --release` and `cargo test --release` green.

## Validation

- Decision: drop `session.rs`. The module was completely unreferenced (no imports from any other file).
- Removed `src/session.rs`, its `mod session;` declaration in `main.rs`, and the `#![allow(dead_code)]` lint override.
- Cleaned up resulting dead-code fallout:
  - Removed `CellStatus::Provisioning` variant (never constructed).
  - Removed `Cell::new` (only used by the deleted `session.rs`).
  - Removed `Cellfile::path()` (unused convenience method).
  - Removed `now_unix()` in `cli.rs` (leftover from session work).
- `cargo build --release` → clean build, zero warnings.
- `cargo test --release` → 35/35 tests pass.

## Notes

- 2026-07-29 created. Not a self-hosting blocker.
- 2026-08-12 implemented.
