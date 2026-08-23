---
id: env-changes-should-not-reprovision
kind: story
parent: v1-self-hostable-hotcell
title: Env changes should not force re-provisioning
status: todo
assignee: null
created: 2026-08-23
updated: 2026-08-23
tags: [cellfile, provisioning]
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
- No task yet; the tech lead should pick an approach first.
