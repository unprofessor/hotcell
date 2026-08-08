---
id: policy-parser-and-matcher-tests
aliases: [policy-parser-and-matcher-tests]
kind: task
parent: firewall-policy-parser-and-matcher
title: Add policy parser and matcher tests
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [firewall, tests, cellfile, phase-1]
depends_on: [implement-host-glob-cidr-matcher]
---

## Goal

Create the compatibility and security test matrix that must pass before the
new policy representation is used for outbound connections.

## Context

The data model and pure matcher are established by the preceding tasks. Test
coverage must catch parser ambiguity—especially IPv6 and legacy serialization
compatibility—before proxy behavior relies on those values.

## Acceptance

- [ ] Legacy `net.allow` files, including optional ports and case-insensitive
  hosts, retain their existing parse and match behavior.
- [ ] Tests reject malformed CIDRs, invalid prefixes/ports, ambiguous or
  malformed IPv6 authorities, empty host patterns, and invalid unsafe escape
  inputs with useful source errors.
- [ ] Tests cover exact/glob apex/subdomain/non-match cases, trailing dots,
  default ports, IPv4 and IPv6 CIDR boundaries, and the approved host/CIDR
  composition table.
- [ ] Tests prove sensitive built-ins win over ordinary allows and that only
  the dedicated unsafe exception changes that outcome.
- [ ] Tests load representative pre-v2 serialized cell declarations and round
  trip v2 declarations without losing policy fields.
- [ ] `cargo test` passes with the new coverage and no test weakens the
  production default simply to reach a local loopback fixture.

## Notes

- 2026-08-08 created as the policy-v2 handoff gate for egress enforcement.
