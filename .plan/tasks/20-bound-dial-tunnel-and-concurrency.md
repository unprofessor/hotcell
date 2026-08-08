---
id: bound-dial-tunnel-and-concurrency
aliases: [bound-dial-tunnel-and-concurrency]
kind: task
parent: firewall-operational-hardening
title: Bound firewall dial, tunnel, and concurrency behavior
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [firewall, timeouts, concurrency, phase-6]
depends_on: [audit-and-warn-tests]
---

## Goal

Prevent a network-enabled cell from consuming unbounded proxy resources while
preserving deterministic client responses and audit events.

## Context

The current firewall has no explicit dial timeout, idle tunnel timeout, or
per-cell concurrent-tunnel cap. It runs on a dedicated Tokio runtime, so
limits need cancellation-safe ownership and release behavior.

## Acceptance

- [ ] The approved configurable/default dial timeout prevents an indefinitely
  pending upstream connection and produces the documented client response and
  audit reason.
- [ ] A configurable/default idle tunnel timeout closes inactive tunnels
  predictably without terminating active byte flow prematurely.
- [ ] A per-cell concurrent-tunnel limit rejects or backpressures excess
  attempts according to a documented status/reason and releases capacity after
  every success, refusal, timeout, upgrade failure, cancellation, and drop.
- [ ] Limits are implemented host-side and do not grant the agent a route or
  move policy into the UDS forwarder.
- [ ] Deterministic tests cover timeout behavior, cap exhaustion, capacity
  release, and at least one active tunnel that remains usable within bounds.
- [ ] Audit remains one decision record per request and accurately records the
  limit/timeout outcome.

## Notes

- 2026-08-08 created. Keep external-network timing out of the test suite.
