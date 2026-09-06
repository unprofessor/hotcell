---
id: workspace-repo-model
kind: story
parent: workspace-sync
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
- The declared/bound matrix (see below).
- Slot name rules: opaque in meaning, constrained in form.
- Opt-in binding of the Cellfile's own repo (leaks its contents into the cell;
  also how a cell improves its own environment behind a review gate).
- Resolve the `seed`/`repo` overlap and record the decision.

The `hotcell bind` surface itself belongs to [[binding-cli-flow]]; this story
owns the file format and semantics it manipulates.

## Declared/bound matrix

| declared | bound | behaviour |
|---|---|---|
| yes | yes | sync |
| yes | no | **error** — declared slots are required |
| no | yes | **error** — no naked bindings |
| no | no | no sync |

**Declared slots are required.** Same shape as an unset `secret`, which already
fails before any sandbox starts; reuse that behaviour and message style. An
`optional=#true` attribute can be added later if authors want a "bind this if
you're also working on it" slot — additive, and backward compatible, whereas
starting optional and tightening later is not.

**No naked bindings.** The Cellfile is not the authority on cell *content* —
bindings are machine-local by design, so two operators already get different
trees. It *is* the authority on cell **shape**, i.e. the set of slots. A
binding with no slot invents shape the Cellfile author cannot reason about
(a provision script written against `/work/hotcell` cannot see it), and
rejecting is recoverable by a one-line Cellfile edit while allowing-then-
restricting is not.

The escape hatch for genuinely ad-hoc work is an ephemeral
`hotcell run --repo <name>=<path>`: rendered in the execution plan, never
persisted, visibly imperative. Persistent config stays declarative and
Cellfile-gated.

## Slot names

Meaning is an out-of-band agreement between author and operator — codenames,
numerals, whatever. hotcell attaches no semantics. Form is constrained, because
the name is load-bearing in four places: the default mount `<workdir>/<name>`,
the key in `bindings.kdl`, the routing token in the git bridge protocol, and
the shadow directory `.cell/<n>/git/<name>.git`. So: safe as a single path
segment, safe as a protocol token, unique within a Cellfile. A name like
`../evil` must be rejected at parse time.

## Acceptance

- [ ] A Cellfile with `repo` slots parses; slot names that are unsafe as a path
  segment or protocol token are rejected at parse time with a clear message.
- [ ] Duplicate slot names in one Cellfile are rejected.
- [ ] Each matrix cell behaves as tabled; both error cells name the offending
  slot and state the fix.
- [ ] `--repo <name>=<path>` binds for one run, appears in `--dry-run` output,
  and writes nothing to `bindings.kdl`.
- [ ] The bindings file is gitignored and never read from the Cellfile's repo
  contents.
- [ ] The Cellfile alone stays portable — no host path in it.
- [ ] The Cellfile's own repo binds only when explicitly opted in.
- [ ] The `seed` decision is recorded and consistent with
  [[packages-seeds-semantics]].
- [ ] Spec matches; `allium check` clean; `cargo fmt --check`, `cargo clippy`,
  `cargo test` clean.

## Notes

- 2026-09-06 filed from the settled design in [[workspace-sync]].
- `.cell/` is gitignored and untracked (verified), so binding the Cellfile's
  own repo does not recursively carry cell state into the shadow.
