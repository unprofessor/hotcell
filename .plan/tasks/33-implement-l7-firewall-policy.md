---
id: implement-l7-firewall-policy
aliases: [implement-l7-firewall-policy]
kind: task
parent: gated-tls-mitm-l7-policy
title: Implement method, path, and header firewall policy
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [firewall, l7, tls, policy, phase-5]
depends_on: [implement-tls-mitm-proxy]
---

## Goal

Implement the approved L7 allow/deny grammar and decisions only on the opt-in
TLS-intercepted path, without expanding into body capture or content mutation.

## Context

CONNECT-only traffic does not reveal HTTP method, path, or headers. After TLS
termination, the policy engine can see those fields, but that visibility is a
privacy-sensitive capability bounded by the approved trust model.

## Acceptance

- [ ] Extend the policy model/guarantees as approved for method, path, and
  header rules while keeping exact Cellfile grammar implementation-defined.
- [ ] Parse and validate the approved L7 rules with deterministic precedence,
  default deny behavior, host/CIDR checks first, and useful source errors.
- [ ] Enforce L7 rules only after verified TLS interception; non-MITM CONNECT
  sessions retain their existing host-level behavior and never claim L7
  enforcement.
- [ ] Audit records a privacy-safe L7 decision reason/rule identifier without
  logging raw headers, authorization values, request bodies, or responses.
- [ ] Header matching follows an explicit normalization/case/repetition policy
  and has bounds that prevent oversized rule/request resource abuse.
- [ ] No body capture, annotation, secret swapping, external authorization
  retry, or response transform is introduced.

## Notes

- 2026-08-08 planned. This task realizes the boundary-style grammar only after
  the trust-model gate and proxy are independently complete.
