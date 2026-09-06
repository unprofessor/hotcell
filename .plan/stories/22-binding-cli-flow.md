---
id: binding-cli-flow
kind: story
parent: workspace-sync
title: CLI flow for reviewing and editing repo bindings
status: todo
assignee: null
created: 2026-09-06
updated: 2026-09-06
tags: [cli, sync, workspace, ergonomics]
depends_on: [workspace-repo-model]
---

## Goal

Let an operator see and change repo bindings without hand-editing
`bindings.kdl` — and see, at a glance, which slots are satisfied.

## Context

Hand-editing the bindings file stays perfectly acceptable; this is the
supported path, not the only one. It matters because bindings are the one
piece of cell configuration that is machine-local and therefore invisible to
the Cellfile author: the operator is the only person who can tell whether a
cell is bindable here, and they need a way to ask.

The declared/bound matrix and the file format belong to
[[workspace-repo-model]]; this story is the surface over them.

## Scope

- **Review.** List every slot with its status against the matrix — bound (and
  to what), declared-but-unbound, or a naked binding with no slot. This is the
  primary command; an operator lands here first on a fresh checkout.
- **Bind / unbind.** Write and remove bindings. A binding command, not a run
  flag, so the declarative model holds.
- **Validate.** Report a binding whose path is missing, is not a repo, or is
  no longer readable — a binding can rot without the Cellfile changing.
- **Actionable errors.** Both matrix error cells surface here as well as at
  run time: name the slot, state the fix, and show the exact command.

Not in scope: the ephemeral `--repo <name>=<path>` run override, which is
deliberately imperative and belongs with the run path in
[[workspace-repo-model]].

## Open

- Whether review output is also machine-readable (a `--json` flag), given the
  CLI's existing habit of keeping stdout parseable and prompts on stderr.
- Whether `bind` on an undeclared name refuses outright or offers the Cellfile
  line to paste. Refusing is consistent; offering is kinder on a fresh repo.

## Acceptance

- [ ] One command shows every slot with its status, and every naked binding.
- [ ] Bind and unbind round-trip through `bindings.kdl` without disturbing
  hand-written formatting or unrelated entries.
- [ ] A rotted binding (path gone, not a repo) is reported as such, distinctly
  from being unbound.
- [ ] Every error names the slot and states the fix.
- [ ] Stdout stays parseable; prompts and diagnostics go to stderr, matching
  `confirm_destroy` (`src/cli.rs:340-347`).
- [ ] `cargo fmt --check`, `cargo clippy`, `cargo test` clean.

## Notes

- 2026-09-06 filed at the developer's request; the `hotcell bind` bullet was
  moved out of [[workspace-repo-model]] so the surface is specified in one
  place.
