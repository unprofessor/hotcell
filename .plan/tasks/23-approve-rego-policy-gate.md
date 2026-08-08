---
id: approve-rego-policy-gate
aliases: [approve-rego-policy-gate]
kind: task
parent: gated-rego-policy
title: Approve the Rego policy extension gate
status: blocked
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [firewall, rego, gate, blocked, phase-4]
depends_on: [hardening-regression-suite]
---

## Goal

Require evidence that the native policy v2 model is insufficient before adding
a policy engine and its dependency/runtime surface.

## Context

Phase 4 is a stretch feature. Core policy v2 is designed to cover normal host,
CIDR, sensitive-address, warn, and audit needs with no Rego dependency. This
ticket is intentionally blocked until a real operator use case exists.

## Acceptance

- [ ] A concrete policy scenario is documented that cannot be expressed safely
  by the native matcher and is valuable enough to justify `regorus`.
- [ ] The requester approves a bounded input/output contract, per-cell policy
  source/ownership, performance budget, memory limits, and failure behavior.
- [ ] The decision states default-deny behavior for missing, malformed,
  compilation-failed, and evaluation-failed policies and how operators learn
  why a request was denied.
- [ ] The decision confirms the core hardening regression gate is done and
  that policy evaluation will remain host-side with no new agent route.
- [ ] A security/maintainability review explicitly approves adding `regorus`
  rather than extending the native matcher; otherwise this task remains
  `blocked` and no dependent implementation work is dispatched.

## Notes

- 2026-08-08 blocked by design. The gate is a product/security decision, not a
  coding spike that may self-approve its own dependency.
