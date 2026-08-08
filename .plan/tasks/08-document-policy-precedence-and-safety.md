---
id: document-policy-precedence-and-safety
aliases: [document-policy-precedence-and-safety]
kind: task
parent: firewall-policy-contract-v2
title: Document policy precedence and sensitive-address safety
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [firewall, policy, security, phase-1]
depends_on: [specify-network-policy-v2]
---

## Goal

Eliminate policy ambiguities before code is written, especially the draft's
contradictory treatment of CIDR allows, denies, and sensitive-address
overrides.

## Context

The product decision requires a distinct explicit unsafe escape. The draft
otherwise leaves unresolved whether host and CIDR rules compose as a union or
intersection, whether CIDRs authorize literal CONNECT targets, and whether
warn mode can bypass an upstream-IP denial.

## Acceptance

- [ ] A versioned implementation-facing policy decision table defines the
  authorization order for exact/glob hosts, port restrictions, allow CIDRs,
  user deny CIDRs, resolved addresses, and literal IP authorities.
- [ ] The table lists the built-in metadata, loopback, and link-local ranges
  for IPv4 and IPv6 and specifies that they win by default.
- [ ] A single prominently named unsafe-exception mechanism is defined;
  ordinary allow-CIDR rules and `warn` cannot override built-ins. Its scope,
  opt-in behavior, and audit visibility are explicit.
- [ ] The decision records whether host and CIDR permissions compose by union
  or intersection and provides representative expected outcomes for each
  supported rule combination.
- [ ] Host canonicalization, default-port behavior, malformed/Unicode host
  handling, and IPv6-literal notation are either specified or deliberately
  rejected with a documented rationale.
- [ ] The decision is consistent with the Allium model from
  `specify-network-policy-v2` and contains no claim that Cellfile grammar is
  normative.

## Notes

- 2026-08-08 created to turn the product choice of an explicit unsafe escape
  into a testable contract rather than an ad hoc parser option.
