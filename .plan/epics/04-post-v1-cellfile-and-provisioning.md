---
id: post-v1-cellfile-and-provisioning
kind: epic
title: Post-V1 Cellfile and provisioning refinements
status: todo
assignee: null
created: 2026-08-24
updated: 2026-08-24
tags: [cellfile, provisioning, post-v1]
---

## Goal

Improve Cellfile capability and provisioning ergonomics beyond the v1
self-hosting bar. Nothing here blocks self-hosting an agent with network
egress; each item is correctness or completeness that is worth doing once the
v1 path ships.

## Scope

- Env changes should not force re-provisioning — story
  `env-changes-should-not-reprovision`. Cell identity currently includes the
  whole agent `environment`, so editing `env.PATH` reproduces an identical
  rootfs from scratch.
- Decide packages/seeds semantics — story `packages-seeds-semantics`. `package`
  and `seed` were dropped from the KDL grammar; this decides whether the
  capability returns and, if so, its grammar.

## Out of scope

- The KDL grammar overhaul and env passthrough/secrets themselves — those are
  the v1 self-hosting bar and stay under [[v1-self-hostable-hotcell]].
- Firewall policy evolution — owned by [[post-v1-firewall-policy-hardening]]
  and [[post-v1-advanced-firewall-policy]].

## Notes

- 2026-08-24 created to rehome two stories out of `v1-self-hostable-hotcell`
  that are not self-hosting blockers, keeping the v1 epic's scope honest.
