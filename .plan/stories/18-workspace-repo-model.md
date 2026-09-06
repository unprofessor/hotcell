---
id: workspace-repo-model
kind: story
parent: post-v1-workspace-sync
title: Cellfile repo slots and machine-local bindings
status: todo
assignee: null
created: 2026-09-06
updated: 2026-09-06
tags: [cellfile, sync, workspace, grammar]
depends_on: [workspace-exchange-contract, packages-seeds-semantics]
---

## Goal

Let a Cellfile declare which repos a cell works on, without putting anyone's
directory layout in a committed file.

## Context

There is no construct for this today — a cell has one `workdir`
(`src/cellfile.rs:212-216`). A `repo` node also overlaps `seed`, which was
dropped from the grammar and is still undecided
(`src/cellfile.rs:239-250`), hence the dependency on
[[packages-seeds-semantics]]: decide both together or relitigate the overlap
later.

## Scope

- Cellfile `repo` node: logical slot, optional `mount` defaulting to
  `<workdir>/<name>`.
- Machine-local bindings file (`.cell/bindings.kdl`), gitignored, mapping slot
  names to host paths.
- `hotcell bind <name> <path>` to write bindings — a binding command, not a run
  flag, so the declarative model holds.
- Resolution: an unbound slot is an error, not a silent clone.
- Opt-in binding of the Cellfile's own repo (leaks its contents into the cell;
  also how a cell improves its own environment behind a review gate).
- Resolve the `seed`/`repo` overlap and record the decision.

## Acceptance

- [ ] A Cellfile with `repo` slots parses; slots with no binding fail with a
  message naming the slot and how to bind it.
- [ ] The bindings file is gitignored and never read from the Cellfile's repo
  contents.
- [ ] The Cellfile alone stays portable — no host path in it.
- [ ] The Cellfile's own repo binds only when explicitly opted in.
- [ ] The `seed` decision is recorded and consistent with
  [[packages-seeds-semantics]].
- [ ] Spec matches; `allium check` clean; `cargo fmt --check`, `cargo clippy`,
  `cargo test` clean.

## Notes

- 2026-09-06 filed from the settled design in [[post-v1-workspace-sync]].
- `.cell/` is gitignored and untracked (verified), so binding the Cellfile's
  own repo does not recursively carry cell state into the shadow.
