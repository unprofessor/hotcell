---
id: define-rego-policy-contract
aliases: [define-rego-policy-contract]
kind: task
parent: gated-rego-policy
title: Define the Rego policy evaluator contract
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [firewall, rego, contract, phase-4]
depends_on: [approve-rego-policy-gate]
---

## Goal

Turn an approved Rego use case into a fail-closed, bounded integration contract
before a policy engine is linked into the firewall.

## Context

If the gate is approved, the policy must evaluate stable firewall facts rather
than raw tunnel data. The source draft proposes input containing host, port,
resolved IP, and method; core audit/decision semantics provide the baseline
native decision and reason.

## Acceptance

- [ ] Define the exact typed Rego input, including normalized authority,
  resolved address, method, native-policy decision, and only privacy-safe
  session/cell metadata.
- [ ] Define a single default-deny output contract, evaluation errors, policy
  versioning, rule/decision identifiers for audit, and composition with native
  sensitive-address protection.
- [ ] Specify when policies load/compile/reload, the bounded cache lifecycle,
  and behavior when a policy source changes during a session.
- [ ] State that Rego cannot override the no-route invariant or sensitive
  built-in protection unless a separately approved explicit unsafe capability
  permits it.
- [ ] Update the Allium model/guarantees only where an optional evaluator is a
  durable semantic promise; do not make a particular Cellfile key normative.
- [ ] Include executable example policies and expected allow/deny outcomes for
  the approved use case.

## Notes

- 2026-08-08 planned after the gate. Do not add `regorus` in this task unless
  the gate's evidence explicitly requires a small validation prototype.
