---
id: implement-firewall-warn-mode
aliases: [implement-firewall-warn-mode]
kind: task
parent: firewall-warn-audit
title: Implement firewall warn-mode enforcement
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [firewall, warn-mode, rollout, phase-3]
depends_on: [implement-session-scoped-jsonl-audit]
---

## Goal

Allow operators to observe ordinary policy misses during rollout while keeping
the default firewall hard-blocking and maintaining the sensitive-address
security boundary.

## Context

The v2 parser exposes per-policy warn state and the audit sink can now show a
decision. Warn mode must not become a general bypass: resolved sensitive IPs,
malformed requests, and upstream failures are not safe to allow merely for
observation.

## Acceptance

- [ ] A policy with warn disabled preserves current hard-block behavior for
  ordinary unmatched host/CIDR requests.
- [ ] With warn enabled, an otherwise ordinary policy miss is allowed through
  only when a permitted resolved address can be selected and the audit action
  is `warn` with the original violation reason.
- [ ] Built-in metadata, loopback, and link-local denials remain hard blocks in
  warn mode; only the dedicated explicit unsafe exception may authorize them.
- [ ] Warn mode does not turn malformed authorities, unsupported methods,
  resolution errors, or unreachable upstreams into successful tunnels.
- [ ] CLI/status output and operator-facing documentation make the active
  observe-only behavior discoverable without exposing policy secrets.
- [ ] Existing default policies remain hard-enforcing with no Cellfile change.

## Notes

- 2026-08-08 created. Detailed behavior and JSON assertions are verified by
  `audit-and-warn-tests`.
