---
id: firewall-resolved-address-enforcement
aliases: [firewall-resolved-address-enforcement]
kind: story
parent: post-v1-firewall-policy-hardening
title: Resolved-address egress enforcement
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [firewall, dns, ssrf, phase-2]
depends_on: [policy-parser-and-matcher-tests]
---

## Goal

Ensure an allowlisted hostname cannot cause Hotcell to tunnel to a forbidden
resolved address, including through DNS rebinding or mixed DNS answers.

## Context

The proxy now admits a CONNECT, returns `200`, and later calls
`TcpStream::connect((host, port))`. That re-resolves the hostname and cannot
return a policy or upstream failure after the successful CONNECT response.
The CLI also starts the bridge only when `allowed_endpoints` is nonempty, which
would incorrectly leave a CIDR-only policy offline.

## Scope

- Activate the proxy for every policy that can permit egress.
- Resolve once, filter resolved addresses, and connect only to the selected
  `SocketAddr` before returning an affirmative CONNECT result.
- Add deterministic security integration coverage without weakening production
  sensitive-address defaults for local test fixtures.

## Notes

- The implementation spine is [[make-network-policy-enable-bridge]] →
  [[resolve-filter-and-dial-once]] → [[return-correct-connect-failures]] →
  [[resolved-address-integration-tests]].
