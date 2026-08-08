---
id: approve-tls-mitm-gate
aliases: [approve-tls-mitm-gate]
kind: task
parent: gated-tls-mitm-l7-policy
title: Approve the TLS MITM trust-model gate
status: blocked
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [firewall, tls, mitm, gate, blocked, phase-5]
depends_on: [hardening-regression-suite]
---

## Goal

Require an explicit security/product decision before giving the host-side
firewall a per-cell certificate authority and visibility into TLS plaintext.

## Context

Current CONNECT-only behavior is boundary enforcement without decryption. TLS
MITM changes that trust model, can break certificate pinning, expands key
management, and requires changes to the sandbox trust configuration.

## Acceptance

- [ ] A concrete L7 need is documented that cannot be met by host/CIDR policy
  and audit alone, including the operator and expected user benefit.
- [ ] An approved threat model covers agent/proxy trust, plaintext exposure,
  CA private-key storage, leaf issuance/cache, compromise response, rotation,
  logging/privacy, and downgrade/rollback behavior.
- [ ] The decision explicitly addresses certificate pinning/unsupported
  clients, trust-store injection, supported TLS protocol behavior, and the
  requirement to preserve a non-MITM fast path when disabled.
- [ ] Security and product owners approve the trust-model change and confirm
  core hardening regression evidence is complete.
- [ ] Without this approval the task remains `blocked`; no CA, rustls, rcgen,
  interception, or L7 implementation ticket may be claimed.

## Notes

- 2026-08-08 blocked by design. This gate cannot be satisfied by an
  implementation agent's unilateral architecture choice.
