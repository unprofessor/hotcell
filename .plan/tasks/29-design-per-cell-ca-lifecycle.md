---
id: design-per-cell-ca-lifecycle
aliases: [design-per-cell-ca-lifecycle]
kind: task
parent: gated-tls-mitm-l7-policy
title: Design the per-cell CA and key lifecycle
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [firewall, tls, ca, design, phase-5]
depends_on: [approve-tls-mitm-gate]
---

## Goal

Turn the approved TLS trust-model decision into a buildable, reviewable
per-cell CA, key, and leaf-certificate lifecycle before cryptographic code is
added.

## Context

MITM requires a private CA key available to the host firewall but never to the
agent, a public trust anchor injected into the sandbox, and a bounded leaf
certificate cache. The design must align with cell state lifecycle and
cleanup/reprovision behavior.

## Acceptance

- [ ] Document the CA/key generation, host-only storage location, permissions,
  ownership, rotation/revocation, destruction, crash recovery, and backup
  posture for each cell/session.
- [ ] Specify leaf issuance inputs, SAN/hostname validation, cache key,
  capacity/TTL/eviction, concurrency model, and failure behavior.
- [ ] Specify how the public CA—not its private key—will be exposed to the
  sandbox trust store, including environment/runtime compatibility and
  certificate-pinning behavior.
- [ ] Select/review the required crypto libraries and versions only after the
  gate; identify their security update/maintenance obligations.
- [ ] Define how disabled MITM preserves the existing CONNECT fast path,
  no-route isolation, audit privacy, and current cell compatibility.
- [ ] Obtain security review of the design before code tasks are dispatched.

## Notes

- 2026-08-08 planned. This is a design/security artifact task, not permission
  to add an unreviewed CA implementation.
