---
id: cellfile-grammar-polish
aliases: [cellfile-grammar-polish]
kind: task
parent: cellfile-kdl-grammar-and-secrets
title: Cellfile grammar polish from the parser review
status: todo
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

- [ ] `provisioning.rs:273` uses the KDL spelling. A grep for the retired
  forms (`provision.script`, `provision.type`, `provision.host_path`,
  `net.allow`, `env.`) across `src/` returns nothing outside string literals
  that intentionally match old-format input.
- [ ] An unsupported KDL type annotation on any node is a source-located
  error rather than silently ignored.
- [ ] `env FOO="a" BAR="b"` produces a message describing what is actually
  wrong.
- [ ] A v1-format Cellfile leads with the migration hint; raw KDL diagnostics
  follow it or are suppressed.
- [ ] Each of the four has a regression test asserting the error text, not
  merely that an error occurred.
- [ ] `cargo fmt`, `cargo clippy --all-targets --all-features` under
  `RUSTFLAGS=-D warnings`, and `cargo test` all pass.

## Notes

- 2026-08-24 created from the parser review's non-blocking findings. Items 3
  and 4 were not independently reproduced when this ticket was written —
  reproduce them first; if either no longer holds, strike it and say so in
  `## Validation` rather than inventing a fix.
