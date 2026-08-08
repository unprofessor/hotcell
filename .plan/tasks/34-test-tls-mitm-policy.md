---
id: test-tls-mitm-policy
aliases: [test-tls-mitm-policy]
kind: task
parent: gated-tls-mitm-l7-policy
title: Test TLS MITM and L7 firewall policy
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [firewall, tls, mitm, l7, tests, phase-5]
depends_on: [implement-l7-firewall-policy]
---

## Goal

Provide independent end-to-end evidence that opt-in MITM/L7 controls are
correct, bounded, and do not regress the core firewall or sandbox guarantees.

## Context

This final gated task crosses CA lifecycle, bwrap trust injection, TLS
handshake, proxy interception, policy matching, audit privacy, and the
non-MITM fast path. Unit tests alone cannot establish those boundaries.

## Acceptance

- [ ] Deterministic local TLS tests prove a trusted per-cell CA handshake and
  correct allow/block outcomes for approved host, method, path, and header
  cases.
- [ ] Tests prove disabled MITM retains the ordinary CONNECT tunnel path and
  legacy Cellfiles do not require a CA, trust-store change, or interception.
- [ ] Tests verify no private CA key is available to the agent, no new route or
  host filesystem exposure exists, and one cell cannot trust/substitute
  another cell's CA material.
- [ ] Certificate-pinning/unsupported-client behavior matches the approved
  gate decision and fails visibly rather than silently weakening enforcement.
- [ ] Audit events show policy outcomes without plaintext bodies, raw headers,
  credentials, or certificate private material.
- [ ] `cargo fmt --check`, `cargo clippy -- -D warnings`, the full suite, and
  repeated local TLS integration tests pass from a clean worktree.

## Notes

- 2026-08-08 planned. An independent reviewer must treat trust-boundary and
  privacy checks as first-class acceptance criteria.
