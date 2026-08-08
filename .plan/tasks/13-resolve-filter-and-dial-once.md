---
id: resolve-filter-and-dial-once
aliases: [resolve-filter-and-dial-once]
kind: task
parent: firewall-resolved-address-enforcement
title: Resolve, filter, and dial each upstream address once
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [firewall, dns, ssrf, phase-2]
depends_on: [make-network-policy-enable-bridge, policy-parser-and-matcher-tests]
---

## Goal

Close the hostname-to-address authorization gap by resolving CONNECT targets
once, applying the resolved-IP policy to those results, and dialing only a
selected returned `SocketAddr`.

## Context

The current `TcpStream::connect((host, port))` performs an implicit hostname
lookup after hostname authorization. It can dial an address different from the
one inspected by a future filter and therefore leaves a DNS-rebinding window.

## Acceptance

- [ ] CONNECT handling performs one explicit async lookup, evaluates every
  returned address through the approved policy decision API, and never invokes
  a hostname-based connect after that lookup.
- [ ] The selected connection uses the actual resolved `SocketAddr`; selection
  behavior for mixed IPv4/IPv6 and multiple permitted answers is deterministic
  and documented.
- [ ] If all answers are denied, no dial is attempted and the caller receives
  the policy-denial path; a sensitive-address denial remains visible to audit.
- [ ] Host/CIDR composition and the dedicated unsafe exception are applied to
  resolved addresses exactly as established by the v2 decision table.
- [ ] Resolver and dial behavior are testable through a deterministic seam or
  fixture without relying on external DNS or editing the host's `/etc/hosts`.
- [ ] Existing tunnel behavior still works for a permitted, reachable address.

## Notes

- 2026-08-08 created. CONNECT response timing and client-visible failures are
  deliberately handled in the following task.
