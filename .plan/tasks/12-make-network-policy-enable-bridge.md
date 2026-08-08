---
id: make-network-policy-enable-bridge
aliases: [make-network-policy-enable-bridge]
kind: task
parent: firewall-resolved-address-enforcement
title: Enable the bridge for every effective network policy
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [firewall, cli, isolation, phase-2]
depends_on: [policy-parser-and-matcher-tests]
---

## Goal

Start the host firewall and UDS bridge whenever policy v2 can authorize an
egress attempt, while preserving the airtight offline path for a genuinely
empty policy.

## Context

`run_cell` currently creates the firewall only when `allowed_endpoints` is
nonempty. A policy that authorizes an address only through an allow CIDR would
otherwise leave the agent offline before its rules can be evaluated.

## Acceptance

- [ ] CLI bridge activation uses a named policy capability/predicate rather
  than directly inspecting `allowed_endpoints`.
- [ ] A valid CIDR-only policy starts the host firewall and its UDS bridge;
  host-based policies retain their current bridge behavior.
- [ ] A policy with no effective egress permission starts neither proxy nor
  bridge and continues to run the agent under `--unshare-net` with no route.
- [ ] The in-namespace forwarder remains a dumb byte pipe; no policy decision
  migrates into `src/isolation.rs` or the agent namespace.
- [ ] Tests cover the empty, legacy-host, and CIDR-only activation paths and
  preserve the existing loopback-only namespace invariant.

## Notes

- 2026-08-08 created. This task intentionally precedes dial-time enforcement
  so CIDR-only policies are not silently unusable.
