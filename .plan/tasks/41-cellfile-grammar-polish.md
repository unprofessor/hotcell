---
id: cellfile-grammar-polish
aliases: [cellfile-grammar-polish]
kind: task
parent: cellfile-kdl-grammar-and-secrets
title: Cellfile grammar polish from the parser review
status: review
assignee: null
created: 2026-08-24
updated: 2026-08-24
tags: [cellfile, grammar, docs, errors]
depends_on: [replace-cellfile-parser-with-kdl]
---

## Goal

Clear the four small non-blocking findings from the
`replace-cellfile-parser-with-kdl` review. None changes behaviour that a
correct Cellfile depends on; all four are quality-of-error or
quality-of-comment issues.

## Context

Recorded in that task's `## Review` section. Two were re-verified on merged
`main` (marked ✓ below); two are taken from the reviewer's report and should
be reproduced before being fixed.

1. **✓ Stale doc comment.** `src/provisioning.rs:273` still describes the
   script as "declared at `provision.script`" — the retired v1 spelling. The
   migration sweep updated `cell.rs` and `isolation.rs` and missed this one.
   Confirmed 2026-08-24 as the only survivor.

2. **✓ KDL type annotations are silently ignored.** `(ty)workdir "/annotated"`
   plans as `--chdir /annotated`; the annotation is dropped without comment.
   Confirmed 2026-08-24. Same family as
   `reject-duplicate-single-valued-nodes`: the parser should not quietly
   discard something the author wrote. An unsupported annotation should be an
   error.

3. **Misleading error on two properties.** `env FOO="a" BAR="b"` (two
   properties, no argument) reportedly hits the "cannot carry both an argument
   and a property" arm, so the message describes a problem the input does not
   have. It is still an error, so fail-closed holds — this is purely about the
   message.

4. **Migration hint ordering.** A v1-format Cellfile emits up to a dozen raw
   KDL diagnostics before the "this looks like the old format" hint. The hint
   is the only line that helps; it should lead.

## Acceptance

- [x] `provisioning.rs:273` uses the KDL spelling. A grep for the retired
  forms (`provision.script`, `provision.type`, `provision.host_path`,
  `net.allow`, `env.`) across `src/` returns nothing outside string literals
  that intentionally match old-format input.
- [x] An unsupported KDL type annotation on any node is a source-located
  error rather than silently ignored.
- [x] `env FOO="a" BAR="b"` produces a message describing what is actually
  wrong.
- [x] A v1-format Cellfile leads with the migration hint; raw KDL diagnostics
  follow it or are suppressed.
- [x] Each of the four has a regression test asserting the error text, not
  merely that an error occurred.
- [x] `cargo fmt`, `cargo clippy --all-targets --all-features` under
  `RUSTFLAGS=-D warnings`, and `cargo test` all pass.

## Notes

- 2026-08-24 created from the parser review's non-blocking findings. Items 3
  and 4 were not independently reproduced when this ticket was written —
  reproduce them first; if either no longer holds, strike it and say so in
  `## Validation` rather than inventing a fix.
- Adjacent finding, not acted on (out of scope): the sibling
  `reject-duplicate-single-valued-nodes` covers duplicate single-valued nodes
  (two `workdir`, two `provision`) still last-winning. Deliberately untouched
  here — a distinct finding from the type-annotation family fixed above.

## Validation

All four findings were reproduced before fixing (findings 3 and 4 were the
two not previously confirmed on this branch; both still held). Reproduction
used a throwaway `#[cfg(test)] mod repro_tmp` in `src/cellfile.rs`, run via
`cargo test --bin hotcell repro_tmp -- --nocapture`, then removed before the
real fixes and regression tests landed.

### Reproductions (before fix)

- Finding 1 — stale comment: `src/provisioning.rs:273` read
  ``declared at `provision.script` ``, the retired v1 dotted spelling.
- Finding 2 — annotations ignored: `parse("(ty)workdir \"/annotated\"")`
  returned `Ok("/annotated")`, and `workdir (ty)"/annotated"` likewise —
  the annotation was silently dropped on both the node and the value.
- Finding 3 — misleading env message: `env FOO="a" BAR="b"` produced
  ``…either a literal `NAME="value"` or a bare `NAME` … not both`` — a
  problem the input (two properties, no argument) does not have.
- Finding 4 — hint ordering: a v1 `key = value` file emitted five raw KDL
  diagnostics (`Expected end of document`, `Found invalid node name`) *before*
  the migration hint on the last line.

### Fixes

1. `src/provisioning.rs:273` now reads ``declared as the `script` property of
   the `provision` node (`provision "shell" script="./provision.sh"`)``.
   `grep -rE 'provision\.(script|type|host_path)|net\.allow|env\.' src/`
   returns only two hits, both inside the raw-string v1 fixture in
   `a_v1_cellfile_gets_a_migration_hint` (intentional old-format input).
2. New `reject_annotations(node, …)` rejects a type annotation on any node or
   any of its values, located at the annotation. Called for every top-level
   node and every child node (`host-path`, `allow`). `KdlIdentifier` gained a
   `Spanned` impl so the error points at the annotation itself.
3. `parse_env` gained two match arms: `([], properties)` for 2+ literals
   (``declares one variable at a time; got N``) and `(arguments, [])` for 2+
   bare names (``passes through one variable at a time; got N``). The
   "not both" arm now fires only when a node truly carries both an argument
   and a property.
4. `syntax_error` now pushes the migration hint first, then the raw
   diagnostics, so the one helpful line leads.

Module doc updated to state that annotations are errors.

### Commands (final, from the worktree)

- `cargo fmt --all -- --check` — clean, exit 0.
- `RUSTFLAGS="-D warnings" cargo clippy --all-targets --all-features` —
  exit 0, zero warnings.
- `cargo test` — exit 0. 26 unit tests (17 in `cellfile`, up from 12: five
  new regression tests) + 46 integration = 72 total, 0 failed, 0 ignored.

### Regression tests added (all assert the error text, not merely failure)

- `a_type_annotation_on_a_node_is_an_error` — `(ty)workdir` → located error.
- `a_type_annotation_on_a_value_is_an_error` — `workdir (ty)"…"` and an
  annotated `port=(u8)80` inside `net`.
- `env_with_two_properties_says_one_at_a_time` — asserts the new message and
  that "not both" is absent.
- `env_with_two_bare_names_says_one_at_a_time` — the mirror case.
- `a_v1_cellfile_leads_with_the_migration_hint` — asserts the hint is the
  first line of the error.
