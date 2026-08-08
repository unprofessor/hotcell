---
id: implement-per-cell-ca-and-leaf-cache
aliases: [implement-per-cell-ca-and-leaf-cache]
kind: task
parent: gated-tls-mitm-l7-policy
title: Implement per-cell CA issuance and leaf cache
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [firewall, tls, ca, cache, phase-5]
depends_on: [design-per-cell-ca-lifecycle]
---

## Goal

Implement the approved host-only CA/key lifecycle and bounded leaf certificate
issuer/cache without yet altering agent trust or intercepting traffic.

## Context

This task follows an approved security design. It is the prerequisite for both
sandbox trust injection and the TLS interception path, and must not store a
private CA key under the agent-visible cell filesystem.

## Acceptance

- [ ] Add only the reviewed crypto dependencies and implement per-cell CA/key
  creation, secure host-state storage, permissions, cleanup, rotation, and
  recovery according to the approved lifecycle.
- [ ] Private key material is never placed in the agent rootfs, Cellfile
  directory, audit stream, process arguments, or ordinary error messages.
- [ ] Leaf certificate issuance validates normalized hostnames/SANs, rejects
  invalid authorities, and uses the approved bounded concurrency-safe cache
  with capacity/expiry/eviction behavior.
- [ ] Failure to read/create/rotate CA material fails safely and produces a
  privacy-safe audit/diagnostic outcome.
- [ ] Unit tests cover lifecycle, permissions where portable, cache reuse and
  eviction, invalid names, cleanup, and concurrent issuance without external
  network access.
- [ ] Core non-MITM sessions remain unchanged when no MITM configuration is
  enabled.

## Notes

- 2026-08-08 planned after CA design approval.
