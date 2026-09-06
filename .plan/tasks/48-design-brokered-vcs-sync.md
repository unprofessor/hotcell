---
id: design-brokered-vcs-sync
aliases: [design-brokered-vcs-sync]
kind: task
parent: design-and-research
title: Design brokered VCS sync between host and cell
status: in_progress
assignee: null
created: 2026-09-05
updated: 2026-09-05
tags: [design, sync, vcs, spike]
depends_on: []
---

## Goal

Converge the brokered VCS sync design far enough to decompose
[[post-v1-workspace-sync]] into implementable tasks.

## Context

In flight as a live design discussion. The current state of the design — the
agreed shape, what is settled, what was rejected and why — is recorded in
[[post-v1-workspace-sync]] and should stay there rather than be duplicated here.
This task tracks the *activity* and gates the implementation story.

## Approach

Answer the open questions, in dependency order. The first was blocking; the
rest shaped the design. All six are now settled or deferred — see the design
record in [[post-v1-workspace-sync]]. Retained below for provenance.

1. **Workspace model** (blocking). How a cell names and mounts N repos. No
   Cellfile construct exists — only a single `workdir`
   (`src/cellfile.rs:212-216`) — and a `repo` node overlaps `seed`, dropped
   from the grammar and still undecided (`src/cellfile.rs:239-250`). Settle
   both together via [[packages-seeds-semantics]].
2. **Is boundary inspection in scope?** Scanning/sanitizing on ingress and
   egress was the original motivation. In scope means the broker needs a
   policy surface and should reuse the decision/audit vocabulary from
   [[firewall-warn-audit]]; out of scope means a dumb pipe and a much smaller
   feature.
3. **Shadow lifecycle.** Whether the bare repo is seeded from the developer's
   real repo at cell start, and how it is refreshed — this is what gives the
   agent real history to rebase against. Plus its fate on `hotcell destroy`.
4. **Shadow remote naming**, including per-cell disambiguation and collision
   with an existing remote of that name.
5. **Backend seam or not.** One concrete broker, or a `SyncBackend`
   enum/trait with the host-side reconciler as a second implementation. A
   plugin system was argued against and should stay rejected absent a gate
   ticket in the `approve-*` style.
6. **Spec first?** `hotcell.allium` has seeding and persistence but no concept
   of *exchange*. Decide whether an open question lands in the spec before any
   code.

## Acceptance

- [x] Each of the six open questions is answered with rationale, or deferred
  with a reason, in the [[post-v1-workspace-sync]] design record.
- [x] The strawman story is retired and its design record lifted to the epic.
- [x] The epic is decomposed into stories with dependency edges:
  [[workspace-exchange-contract]], [[workspace-repo-model]],
  [[git-bridge-transport]], [[shadow-repo-lifecycle]],
  [[shadow-ref-protection]].
- [x] Decisions that change existing surfaces are cross-referenced from the
  tickets that own them ([[rename-destroy-force-to-no-confirm]],
  [[packages-seeds-semantics]], [[guard-uds-socket-path-length]]).
- [ ] Decomposition reviewed by the developer; close once accepted.

## Notes

- 2026-09-05 opened in_progress; the design discussion was already underway
  when this was filed.
