---
id: specify-network-policy-v2
aliases: [specify-network-policy-v2]
kind: task
parent: firewall-policy-contract-v2
title: Specify NetworkPolicy v2 model and guarantees
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [firewall, specification, policy, phase-1]
depends_on: []
---

## Goal

Make `hotcell.allium` the agreed normative source for policy v2 semantics
before changing the Cellfile representation or proxy behavior.

## Context

The specification currently exposes only `allowed_endpoints: Set<Endpoint>`
and says the agent can reach only endpoints in that policy. The requested
model adds hostname patterns, resolved-IP policy, built-in sensitive-address
denial, a deliberately unsafe exception, warn mode, and audit decisions.
Cellfile parsing remains explicitly outside the formal grammar.

## Acceptance

- [ ] `NetworkPolicy` and the `NetworkFirewall` guarantee model exact/glob
  host authorization, optional ports, CIDR authorization, resolved-address
  enforcement, sensitive-address protection, a distinct unsafe exception,
  warn behavior, and auditable decisions.
- [ ] The specification preserves: empty policy means no network access; the
  agent has no route; the host-side proxy is the enforcement point; and legacy
  endpoint declarations remain supported.
- [ ] The specification states that ordinary allow rules and warn mode cannot
  implicitly re-authorize metadata, loopback, or link-local targets.
- [ ] The exact dotted-key Cellfile syntax is not added to Allium; an
  implementation-facing note may link the model to parser work without making
  grammar normative.
- [ ] The changed specification is internally consistent with existing
  surfaces/guarantees and passes the repository's applicable spec validation
  or documented manual review.

## Notes

- 2026-08-08 created. This task defines model/guarantees only; precedence and
  implementation grammar are deliberately delegated to the next task.
