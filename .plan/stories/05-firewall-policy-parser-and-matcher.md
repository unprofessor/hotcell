---
id: firewall-policy-parser-and-matcher
aliases: [firewall-policy-parser-and-matcher]
kind: story
parent: post-v1-firewall-policy-hardening
title: Firewall policy parser and matcher
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [firewall, cellfile, parser, matcher, phase-1]
depends_on: [document-policy-precedence-and-safety]
---

## Goal

Make the approved policy v2 representable, backward-compatible, and
unambiguously matchable for hostnames and resolved IPv4/IPv6 addresses.

## Context

`src/cellfile.rs` currently parses only `host[:port]` using `rsplit_once(':')`,
which cannot correctly represent host-only IPv6 literals. Its serialized
`NetworkPolicy` is stored in `provisioned_as.json`, so new fields need defaults
when old cells are loaded. `src/firewall.rs` currently performs an exact,
case-insensitive hostname comparison.

## Scope

- Extend types, Cellfile parsing, and serde compatibility.
- Provide normalized exact/glob hostname and CIDR matching according to the
  contract, including IPv6 behavior.
- Build unit coverage before policy enforcement starts dialing addresses.

## Notes

- [[extend-network-policy-types-and-persistence]] establishes the data model;
  [[implement-host-glob-cidr-matcher]] supplies matching; and
  [[policy-parser-and-matcher-tests]] is the handoff gate.
