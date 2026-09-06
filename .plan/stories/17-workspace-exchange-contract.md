---
id: workspace-exchange-contract
kind: story
parent: workspace-sync
title: Workspace exchange contract in Allium
status: todo
assignee: null
created: 2026-09-06
updated: 2026-09-06
tags: [spec, allium, sync, contract]
depends_on: []
---

## Goal

Define workspace exchange in `hotcell.allium` before any of it is built.

## Context

The spec has *seeding* (`FileMapping`, one-shot, ingress-only,
`hotcell.allium:61-64`) and *persistence* (`FilesPersistAfterSession`, 476-482)
but no concept of **exchange** — nothing describes source crossing the boundary
in both directions, or a repo the host and cell both hold.

hotcell is spec-first: this story writes the contract the other four
implement against, and it gates all of them.

## Scope

- A repo/slot entity and its binding, distinguishing the committed declaration
  from the machine-local binding.
- The shadow lifecycle as a Birth/Death inverse pair, including the data-loss
  precondition that makes Death a valid inverse of Birth.
- Guarantees: what exchange does and does not promise, and how it interacts
  with `FilesystemIsolation` (it must not weaken it — the agent still touches
  no host path; bytes cross via a mediated channel).
- Where `destroy` delegates, and the confirmation/refusal split.

## Out of scope

- Boundary inspection (scanning, sanitizing). Deliberately unspecified so the
  contract does not have to change when it arrives.
- Anything in the epic's Deferred list.

## Acceptance

- [ ] `hotcell.allium` describes exchange, the repo slot/binding split, and the
  shadow lifecycle.
- [ ] `allium check hotcell.allium` reports 0 errors.
- [ ] No guarantee is weakened silently — any change to an existing guarantee
  is called out explicitly with rationale.
- [ ] The four implementing stories can be read against the spec without
  needing this discussion.

## Notes

- 2026-09-06 filed from the settled design in [[workspace-sync]].
