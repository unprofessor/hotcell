---
id: bound-audit-lifecycle
aliases: [bound-audit-lifecycle]
kind: task
parent: firewall-operational-hardening
title: Bound the firewall audit lifecycle
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [firewall, audit, rotation, phase-6]
depends_on: [bound-dial-tunnel-and-concurrency]
---

## Goal

Make the dedicated firewall audit log predictable and bounded across repeated
sessions without violating the current session's decision-record contract.

## Context

The audit implementation intentionally establishes a durable session-scoped
file first. Without rotation or a size policy, a long-lived or repeatedly run
cell can grow unbounded state outside its sandbox rootfs.

## Acceptance

- [ ] The implementation documents a session-start rotation/naming policy,
  maximum retained size/segment count, file permissions, and cleanup behavior.
- [ ] Starting a new session cannot append ambiguously to a prior session's
  audit stream; retained files remain identifiable enough for operator review.
- [ ] Size enforcement/rotation preserves complete JSON lines and keeps the
  current active session capable of writing one event per later request.
- [ ] Cleanup/rotation failures have a documented safe behavior and do not
  silently mix audit data between cells.
- [ ] Deterministic tests exercise rotation, size boundaries, retained
  parseability, and fresh-session isolation without relying on wall-clock
  sleeps or external storage.
- [ ] Audit storage remains host-owned and inaccessible from inside the agent
  sandbox.

## Notes

- 2026-08-08 created after concurrency limits to avoid concurrent lifecycle
  redesigns in the same firewall/audit code paths.
