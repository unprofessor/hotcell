---
id: implement-host-glob-cidr-matcher
aliases: [implement-host-glob-cidr-matcher]
kind: task
parent: firewall-policy-parser-and-matcher
title: Implement normalized host glob and CIDR matcher
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [firewall, matcher, cidr, phase-1]
depends_on: [extend-network-policy-types-and-persistence]
---

## Goal

Implement pure, reusable policy-matching primitives that exactly follow the
approved v2 decision table and can be tested without dialing a network socket.

## Context

The existing matcher is an exact case-insensitive hostname comparison in
`src/firewall.rs`. Matching must now normalize authorities, distinguish glob
semantics from suffix matching, and evaluate IPv4/IPv6 CIDRs without allowing
a partially normalized host or address through.

## Acceptance

- [ ] Host normalization implements the approved case, trailing-dot, default
  port, malformed-authority, Unicode/IDNA, and IPv6-literal rules exactly
  once at the policy boundary.
- [ ] Exact and anchored glob matching implements the documented `*` and `?`
  semantics; `*.example.com` has the approved apex/subdomain behavior and
  cannot match `evil-example.com`.
- [ ] CIDR parsing and matching cover IPv4 and IPv6 prefix boundaries and
  apply the approved host/CIDR composition and deny precedence.
- [ ] Built-in sensitive ranges and the distinct unsafe exception are exposed
  through a clear result/decision type; ordinary allow rules do not conceal
  why an address was denied.
- [ ] The matcher has no DNS lookup, socket I/O, global mutable policy state,
  or dependence on Cellfile source text.
- [ ] Unit tests cover the new pure matcher API and pass under `cargo test`.

## Notes

- 2026-08-08 created. Resolve-once behavior remains the responsibility of
  `resolve-filter-and-dial-once`.
