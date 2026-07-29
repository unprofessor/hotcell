---
id: remove-dead-code
kind: task
parent: polish-and-docs
title: Remove dead-code allow and decide on session wiring
status: todo
assignee: null
created: 2026-07-29
updated: 2026-07-29
tags: []
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

## Notes

- 2026-07-29 created. Not a self-hosting blocker.
