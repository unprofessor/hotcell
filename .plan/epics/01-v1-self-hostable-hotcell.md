---
id: v1-self-hostable-hotcell
kind: epic
title: Ship v1 self-hostable hotcell
status: todo
assignee: null
created: 2026-07-29
updated: 2026-07-29
tags: []
---

## Goal

Ship v1 self-hostable hotcell

## Scope

Make hotcell capable of self-hosting an AI coding agent (e.g. Pi) that needs
network egress to a model provider. Today the offline path works end-to-end
(provisioning, isolation, state, CLI, tests); the one v1 gap is agent network
egress — the firewall is a stub, and `cli.rs` refuses to run any cell with a
non-empty `net.allow` policy.

- Network firewall (HTTP allowlist proxy) — story `network-firewall`.
- Polish and docs — story `polish-and-docs`.
- Decide packages/seeds semantics — story `packages-seeds-semantics`.

## Out of scope

- Billing, quota, resource limits (spec-excluded).
- Authentication/authorization of callers (spec-excluded).
- Choice of agent program (hotcell runs any executable).
- Cellfile format evolution (implementation detail).
- Port/protocol/path-level network rules beyond hostname allowlist (open
  question in `hotcell.allium`; v1 = hostname/domain allowlist is sufficient).

## Notes

- 2026-07-29 created. Seeded from a gap analysis: `cargo build --release`
  and all 21 tests pass; the only `TODO(v1)` is `src/firewall.rs`.
