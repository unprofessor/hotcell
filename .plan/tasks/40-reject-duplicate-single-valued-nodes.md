---
id: reject-duplicate-single-valued-nodes
aliases: [reject-duplicate-single-valued-nodes]
kind: task
parent: cellfile-kdl-grammar-and-secrets
title: Reject duplicate single-valued Cellfile nodes
status: todo
assignee: null
created: 2026-08-24
updated: 2026-08-24
tags: [cellfile, grammar, fail-closed]
depends_on: [replace-cellfile-parser-with-kdl]
---

## Goal

Make a repeated single-valued node a source-located error instead of silently
discarding everything but the last occurrence.

## Context

Surfaced by the reviewer of `replace-cellfile-parser-with-kdl` as a
non-blocking finding, then reproduced on merged `main`:

```kdl
provision "shell" script="./p.sh" { host-path "/etc/hostname" }
provision "shell" script="./p.sh" { host-path "/etc/machine-id" }
```

plans as:

```
--ro-bind /etc/machine-id /hotcell/host/etc/machine-id
```

`/etc/hostname` is gone. Exit 0, no warning, nothing in the plan to suggest a
declaration was dropped. Duplicate `workdir` behaves the same way — the second
silently wins.

This is **not a regression**: the v1 parser last-wins identically, which is
why the reviewer correctly declined to block the merge on it. But every other
ambiguity in the KDL grammar is now an error — unknown nodes, unknown
properties, wrong arity, an `env` node carrying both an argument and a
property. Silently discarding a declaration the author wrote is the one
remaining place the parser guesses, and in a sandbox tool a dropped
`host-path` or a dropped `net` block is exactly the kind of guess that
produces a Cellfile whose behaviour does not match its text.

## Approach

The distinction that matters, and which an implementer must not get wrong:

- **Single-valued nodes** — `workdir`, `provision`, `net`. A second occurrence
  is an error.
- **Multi-valued nodes** — `env`, `secret`, and the children `host-path` and
  `allow`. These accumulate by design and **must keep accumulating**. Breaking
  that would be a far worse bug than the one this task fixes.

## Acceptance

- [ ] A repeated `workdir`, `provision` or `net` node is an error naming the
  node and citing both occurrences' `line:col`, not just the second.
- [ ] `env`, `secret`, `host-path` and `allow` still accumulate; a regression
  test asserts multiple `env` and multiple `allow` nodes all survive.
- [ ] Duplicate `net` blocks specifically are covered — confirm first whether
  the second block currently replaces the first's `allow` children or merges
  them. This was not observable via `--dry-run` (the endpoint count prints
  only at real run time), so establish the current behaviour before changing
  it, and record what you found.
- [ ] No state is created when the error fires; `--dry-run` fails identically.
- [ ] `cargo fmt`, `cargo clippy --all-targets --all-features` under
  `RUSTFLAGS=-D warnings`, and `cargo test` all pass.

## Notes

- 2026-08-24 created. **Decision recorded, not discovered:** rejecting
  duplicates was chosen over keeping last-wins, for consistency with the rest
  of the grammar's fail-closed stance. The alternative — keep last-wins and
  document it — is defensible if a Cellfile include/override mechanism is ever
  added, since override semantics would then be useful. No such mechanism
  exists or is planned, so the error is the better default today.
