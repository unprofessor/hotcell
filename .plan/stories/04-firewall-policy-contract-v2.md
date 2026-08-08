---
id: firewall-policy-contract-v2
aliases: [firewall-policy-contract-v2]
kind: story
parent: post-v1-firewall-policy-hardening
title: Firewall policy v2 contract
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [firewall, policy, specification, phase-1]
depends_on: []
---

## Goal

Define the security semantics that code, Cellfile parsing, and audit behavior
must share before the policy representation changes.

## Context

`hotcell.allium` currently models only `allowed_endpoints: Set<Endpoint>` and
states a hostname-level firewall guarantee. The requested policy adds host
globs, CIDRs, sensitive-address safety, an unsafe exception, warn mode, and
audit decisions while retaining the format exclusion for Cellfile syntax.

## Scope

- Update the normative model and guarantees without embedding dotted-key
  Cellfile syntax in Allium.
- Decide and document the policy algebra and every security-sensitive
  precedence rule before parser or proxy work begins.
- Preserve the empty-policy and no-route invariants from the parent epic.

## Notes

- Work starts with [[specify-network-policy-v2]] and is completed by
  [[document-policy-precedence-and-safety]].
