---
id: post-v1-advanced-firewall-policy
aliases: [post-v1-advanced-firewall-policy]
kind: epic
title: Post-V1 advanced firewall policy (phases 4–5)
status: blocked
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [firewall, post-v1, gated, stretch]
depends_on: [hardening-regression-suite]
---

## Goal

Keep a concrete, reviewable path for optional Rego policy and TLS MITM/L7
controls without allowing either trust-model change to enter the core firewall
hardening delivery by accident.

## Gate

This epic is intentionally blocked. Its work begins only after the core
firewall hardening regression gate is done **and** the applicable child gate
has an independently reviewed affirmative decision. A completed gate must
record the concrete user need, threat model, operational budget, and rollback
plan; absence of a use case means the feature remains blocked.

## Scope

- [[gated-rego-policy]]: optional compiled/cached `regorus` evaluation for
  policy composition that cannot be expressed by the native matcher.
- [[gated-tls-mitm-l7-policy]]: an explicit CA/trust-model change, CONNECT TLS
  interception, and method/path/header-level policy only after approval.

## Out of scope

- Preemptively adding `regorus`, `rustls`, `rcgen`, or related dependencies.
- Changing the no-route sandbox model, transparent proxying, DNS hijacking,
  request-body capture, or external audit collection without a later plan.

## Notes

- 2026-08-08 created as the second of two requested post-V1 epics. The
  `blocked` state is deliberate: the included tickets make scope and gates
  explicit, not approved implementation work.
