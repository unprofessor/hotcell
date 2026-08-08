---
id: resolved-address-integration-tests
aliases: [resolved-address-integration-tests]
kind: task
parent: firewall-resolved-address-enforcement
title: Add resolved-address enforcement integration tests
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [firewall, tests, dns, ssrf, phase-2]
depends_on: [return-correct-connect-failures]
---

## Goal

Prove the complete proxy path enforces resolved-address policy and response
semantics under deterministic local fixtures.

## Context

Pure matcher tests cannot demonstrate that an allowed hostname is not
re-resolved or that an HTTP client receives a refusal before CONNECT success.
Existing loopback echo tests must not cause production default sensitive-range
protection to be weakened.

## Acceptance

- [ ] Integration tests use an injected resolver/dial seam or deterministic
  local test harness; they do not depend on external DNS, a public upstream,
  or edits to `/etc/hosts`.
- [ ] An allowlisted name resolving only to a sensitive address is refused
  without a dial, while the same fixture succeeds only with the dedicated
  test-policy unsafe exception.
- [ ] A mixed answer set proves denied addresses are skipped and an approved
  address is dialed by its returned `SocketAddr`, with no second hostname
  lookup.
- [ ] Tests cover IPv4 and IPv6 sensitive ranges, an ordinary permitted
  address, all-addresses-denied, lookup failure, and allowed-but-refused
  upstream status behavior.
- [ ] The suite verifies the empty-policy offline path and existing
  loopback-only sandbox guarantees still hold.
- [ ] `cargo test` passes repeatedly for the new tests; any intentionally
  network-sensitive test documents its repeat count and local fixture.

## Notes

- 2026-08-08 created as the resolved-address handoff gate for audit and
  operational work.
