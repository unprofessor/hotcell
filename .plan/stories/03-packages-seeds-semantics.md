---
id: packages-seeds-semantics
kind: story
parent: v1-self-hostable-hotcell
title: Decide packages/seeds semantics
status: todo
assignee: null
created: 2026-07-29
updated: 2026-07-29
tags: []
---

## Goal

Decide packages/seeds semantics

## Context

Parent: `v1-self-hostable-hotcell`. The Cellfile parser (`src/cellfile.rs`)
parses `package =` and `seed = source => target` into `CellDeclaration`, but
no built-in provisioner acts on them — the `shell` provisioner is expected to
do that work itself, and `hotcell.allium` treats packages/seeds as declaration
content the provisioner interprets. Decide and document the v1 contract:

- Are `package`/`seed` advisory (provisioner-authored scripts read them) or is
  there a built-in provisioner kind that installs them?
- If advisory, should hotcell warn/error when they're set under `kind = none`?

## Notes

- 2026-07-29 created. Not a self-hosting blocker; deferred from the firewall
  story. No task yet — the tech lead should split this after the firewall
  lands.
