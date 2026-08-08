---
id: implement-regorus-policy-evaluator
aliases: [implement-regorus-policy-evaluator]
kind: task
parent: gated-rego-policy
title: Implement the cached regorus policy evaluator
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [firewall, rego, regorus, phase-4]
depends_on: [define-rego-policy-contract]
---

## Goal

Implement the approved Rego evaluator as a host-side, compiled and cached
component with fail-closed behavior.

## Context

The core firewall owns a Tokio runtime and handles multiple connections. Rego
must not be parsed or compiled per request, must not receive decrypted tunnel
payloads, and must integrate with the established decision/audit model.

## Acceptance

- [ ] Add the approved `regorus` dependency only after the gate/contract are
  complete and keep the dependency surface minimal and documented.
- [ ] Implement a dedicated policy module with compilation, typed input/output
  mapping, bounded per-cell cache ownership, and clear lifecycle/drop behavior.
- [ ] Missing policy, parse/compile failure, evaluation failure, timeout/budget
  exhaustion, and unexpected output fail closed with stable auditable reasons.
- [ ] Evaluation never occurs before non-overridable native sensitive-address
  protections and never grants the agent a direct route or forwarder logic.
- [ ] Unit tests prove default deny, known allow/deny decisions, malformed
  policy handling, cache reuse, and safe handling of untrusted policy content.
- [ ] The module exposes no network, filesystem, or raw request-body capability
  beyond the approved policy-source loading boundary.

## Notes

- 2026-08-08 planned; implementation remains blocked until its prerequisite
  gate is actually approved and closed.
