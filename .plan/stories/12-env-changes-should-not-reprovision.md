---
id: env-changes-should-not-reprovision
kind: story
parent: post-v1-cellfile-and-provisioning
title: Env changes should not force re-provisioning
status: todo
assignee: null
created: 2026-08-23
updated: 2026-08-24
tags: [cellfile, provisioning, identity]
depends_on: [workspace-exchange-contract]
---

## Goal

Stop treating the agent's environment as part of cell identity, so editing
`env.PATH` does not trigger a full re-provision.

## Context

`Cell::is_current` compares whole `CellDeclaration` values, and
`CellDeclaration` includes `environment`. But provisioning consumes exactly
one thing from the agent env: `provisioning.rs:181` searches it for `HOME`, to
set the provisioner's own HOME so tool caches land inside the cell. Everything
else in that vector is read only at `plan.rs:196`, when building the agent
step.

So any env edit invalidates the cell and forces the provisioner to run again
from scratch, reproducing an identical rootfs.

Three approaches were sketched; none chosen:

1. Exclude `environment` from identity except `HOME`. Cheapest, but the HOME
   special case is a smell.
2. Give provisioning its own declared home (`provision "shell" home="..."`),
   decoupling it from the agent's `env.HOME`, after which `environment` drops
   out of identity cleanly. Largest scope, cleanest result.
3. Leave it. Wasteful but correct.

## Notes

- 2026-08-23 created. Deferred deliberately from
  `cellfile-kdl-grammar-and-secrets` to keep that change reviewable — bundling
  would land two behaviour changes in one commit.
- Worth doing soon after: that story already invalidates every existing cell
  by changing the declaration shape, so a second identity change is cheap
  while few cells exist.
- 2026-08-24 approach chosen: **#2** (give provisioning its own declared
  home, e.g. `provision "shell" home="..."`, so `environment` drops out of
  cell identity cleanly). #1 is a hack around the `HOME` special case; #3
  leaves the waste in place. No task yet — needs the grammar for the
  provisioner `home` property, which lands with
  [[cellfile-kdl-grammar-and-secrets]].
- 2026-08-24 rehomed under [[post-v1-cellfile-and-provisioning]]: a caching
  refinement, not a self-hosting blocker.
- 2026-09-06 the mechanism is now **specified**, not just sketched.
  [[workspace-exchange-contract]] introduced
  `CellDeclaration.provisioning_identity` — the subset of the declaration that
  determines the rootfs — and redefined `is_current` over it, excluding both
  `environment` and `repos`. Workspace sync needed the same property for repo
  slots, so rather than special-casing twice, one projection now covers both.
  This story implements it.
- 2026-09-06 approach #2 stands and is now load-bearing: excluding the whole
  environment from identity is only sound once nothing in provisioning reads
  that environment, so decoupling the provisioner's home from the agent's
  `env.HOME` is a prerequisite of the specified behaviour rather than a
  tidiness preference. The spec records the residual question — whether any
  other provisioning input is drawn from the agent's declared environment —
  as an open question; answer it here.
