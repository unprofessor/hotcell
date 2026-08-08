---
id: audit-and-warn-tests
aliases: [audit-and-warn-tests]
kind: task
parent: firewall-warn-audit
title: Add firewall audit and warn-mode tests
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [firewall, audit, warn-mode, tests, phase-3]
depends_on: [implement-firewall-warn-mode]
---

## Goal

Lock down the audit cardinality, schema, privacy, and warn-mode safety
semantics with deterministic tests.

## Context

Audit and warn behavior cross policy parsing, resolved-address enforcement,
HTTP response timing, and concurrent file writes. This task is the stable
handoff before resource limits and log lifecycle changes.

## Acceptance

- [ ] Tests issue allowed, ordinary-blocked, warned, sensitive-blocked,
  unsafe-exception, malformed, unsupported-method, resolution-failure, and
  upstream-failure requests against deterministic fixtures.
- [ ] Every request produces exactly one parseable JSONL event with required
  action/reason/rule/address fields appropriate to its outcome.
- [ ] Tests verify `duration_ms` follows the documented measurement scope and
  events contain no request body, headers, credentials, or tunnel payload.
- [ ] A warned ordinary miss reaches an allowed fixture; warn mode cannot reach
  a built-in sensitive fixture without the explicit unsafe exception.
- [ ] Concurrent requests preserve one-whole-JSON-object-per-line behavior and
  do not lose or duplicate events.
- [ ] The full relevant test suite passes without external provider credentials
  or public network access.

## Notes

- 2026-08-08 created as the audit/warn handoff gate.
