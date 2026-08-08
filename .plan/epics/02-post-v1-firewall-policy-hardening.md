---
id: post-v1-firewall-policy-hardening
aliases: [post-v1-firewall-policy-hardening]
kind: epic
title: Post-V1 firewall policy hardening (phases 1–3 and 6)
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [firewall, post-v1, security]
depends_on: []
---

## Goal

Evolve Hotcell's CONNECT-only hostname allowlist into a hardened, observable
host-side egress firewall without weakening the agent's no-route isolation.

## Scope

- Policy v2: exact and glob host rules, CIDR rules, sensitive-address
  protection, an explicit unsafe exception, and a per-cell warn mode.
- Resolve-once, resolved-address filtering and dialing to close the current
  DNS-rebinding/SSRF gap.
- One structured audit event per firewall decision, session-scoped JSONL
  storage, rollout-safe warn behavior, and operational limits.
- Backward-compatible `net.allow = host[:port]` parsing and compatible loading
  of pre-v2 provisioned declarations.
- Specification changes to the `NetworkPolicy` model and `NetworkFirewall`
  guarantees in `hotcell.allium`; the exact Cellfile grammar remains an
  implementation detail.

## Out of scope

- Rego evaluation, TLS MITM, CA injection, L7 method/path/header policy, body
  inspection, and response transforms. These are owned by
  [[post-v1-advanced-firewall-policy]].
- Any network route for the agent, transparent-proxy/DNS-hijack architecture,
  or changes to the UDS bridge's dumb-byte-pipe role.

## Invariants

1. An empty policy starts no proxy or bridge and leaves the agent fully offline.
2. The agent remains in `bwrap --unshare-net`; all enforcement is host-side.
3. Legacy Cellfiles keep their existing behavior.
4. Metadata, loopback, and link-local targets remain denied unless a distinct,
   prominently unsafe policy exception is configured; warn mode cannot bypass
   that protection.

## Sequencing

Implementation proceeds through [[firewall-policy-contract-v2]],
[[firewall-policy-parser-and-matcher]],
[[firewall-resolved-address-enforcement]], [[firewall-warn-audit]], and
[[firewall-operational-hardening]]. The final regression task is the readiness
gate for the advanced epic.

## Notes

- 2026-08-08 created from `2026-08-06_013244-firewall-growth.md` after product
  decisions: phases 1–3 and 6 ship here; sensitive ranges require an explicit
  unsafe escape; Allium specifies model/guarantees, not Cellfile grammar.
