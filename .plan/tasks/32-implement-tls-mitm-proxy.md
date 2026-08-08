---
id: implement-tls-mitm-proxy
aliases: [implement-tls-mitm-proxy]
kind: task
parent: gated-tls-mitm-l7-policy
title: Implement gated TLS MITM CONNECT interception
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [firewall, tls, mitm, proxy, phase-5]
depends_on: [implement-per-cell-ca-and-leaf-cache, inject-ca-trust-into-agent-sandbox]
---

## Goal

Add an opt-in TLS termination/interception path using the approved CA while
retaining the current CONNECT tunnel behavior as the fast, default path.

## Context

The current proxy acknowledges CONNECT and copies encrypted bytes. L7 controls
require a separate rustls-based path that presents a leaf certificate to the
agent and establishes the upstream TLS connection without exposing plaintext
outside the approved host-side policy/audit boundary.

## Acceptance

- [ ] MITM activation is explicit, configuration-validated, and unavailable
  unless the TLS gate and CA/trust prerequisites are complete.
- [ ] The interception path issues/uses a validated per-cell leaf certificate,
  terminates agent TLS, creates upstream TLS safely, and returns deterministic
  client-facing failures for certificate/handshake/upstream errors.
- [ ] With MITM disabled, the existing CONNECT-only byte-tunnel path remains
  behaviorally/performance-compatible and does not create certificates.
- [ ] Native host/CIDR and sensitive-address policy still runs before an
  upstream connection; interception grants no route or forwarder bypass.
- [ ] Plaintext, headers, and request bodies are not logged or retained merely
  by enabling TLS interception.
- [ ] Deterministic tests cover trusted handshake success, disabled fast path,
  invalid authority/certificate behavior, CA isolation, and no external
  network dependency.

## Notes

- 2026-08-08 planned. Method/path/header controls are explicitly deferred to
  the next task once safe TLS termination exists.
