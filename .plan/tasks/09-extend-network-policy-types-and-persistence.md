---
id: extend-network-policy-types-and-persistence
aliases: [extend-network-policy-types-and-persistence]
kind: task
parent: firewall-policy-parser-and-matcher
title: Extend network policy types, Cellfile parsing, and persistence
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [firewall, cellfile, serde, phase-1]
depends_on: [document-policy-precedence-and-safety]
---

## Goal

Represent the approved policy v2 safely in Rust, parse it from additive
Cellfile keys, and load existing provisioned cells without deserialization
failures or behavior regressions.

## Context

`src/cellfile.rs` currently stores only `Vec<Endpoint>` and parses host/port
with `rsplit_once(':')`. The declaration is serialized in
`.cell/<name>/provisioned_as.json`, so newly added fields must have safe serde
defaults for old records. The CLI will later need a predicate that recognizes
CIDR-only policies as effective egress policies.

## Acceptance

- [ ] `NetworkPolicy` has an explicit v2 representation for host rules,
  CIDR allow/deny rules, the approved unsafe exception, and per-policy warn
  state; every new persisted field has a safe serde default.
- [ ] Existing Cellfiles using `net.allow = host[:port]` parse to equivalent
  behavior, and pre-v2 `provisioned_as.json` data loads successfully.
- [ ] Additive implementation-defined keys parse the approved v2 grammar,
  including unambiguous IPv6 literals; malformed CIDRs, ports, host patterns,
  and unsafe-exception values return source-located errors.
- [ ] The type/API exposes the minimal policy capability needed for the CLI to
  determine whether a bridge must start, without treating an empty policy as
  network-enabled.
- [ ] Parser errors do not silently discard an unrecognized network rule, and
  serialization/deserialization retains all v2 policy data.
- [ ] Focused parser and persistence tests pass; broader matcher coverage is
  owned by `policy-parser-and-matcher-tests`.

## Notes

- 2026-08-08 created. Do not implement proxy matching or DNS resolution here.
