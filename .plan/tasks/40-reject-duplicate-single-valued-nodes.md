---
id: reject-duplicate-single-valued-nodes
aliases: [reject-duplicate-single-valued-nodes]
kind: task
parent: cellfile-kdl-grammar-and-secrets
title: Reject duplicate single-valued Cellfile nodes
status: review
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

- [x] A repeated `workdir`, `provision` or `net` node is an error naming the
  node and citing both occurrences' `line:col`, not just the second.
- [x] `env`, `secret`, `host-path` and `allow` still accumulate; a regression
  test asserts multiple `env` and multiple `allow` nodes all survive
  (`multi_valued_nodes_still_accumulate`).
- [x] Duplicate `net` blocks specifically are covered — established current
  behaviour before changing it: two `net` blocks **merged** their `allow`
  children into a single allowlist (both survived), not replace. Confirmed by
  a throwaway reproduction test before the fix (recorded in Validation). Now
  the second `net` block is a located error like the others.
- [x] No state is created when the error fires; `--dry-run` fails identically.
  Verified by `a_duplicate_single_valued_node_fails_and_creates_no_state`,
  which asserts non-zero exit and that no `.cell` directory is created — the
  error is raised while reading the Cellfile, before any state exists.
- [x] `cargo fmt`, `cargo clippy --all-targets --all-features` under
  `RUSTFLAGS=-D warnings`, and `cargo test` all pass.

## Notes

- 2026-08-24 created. **Decision recorded, not discovered:** rejecting
  duplicates was chosen over keeping last-wins, for consistency with the rest
  of the grammar's fail-closed stance. The alternative — keep last-wins and
  document it — is defensible if a Cellfile include/override mechanism is ever
  added, since override semantics would then be useful. No such mechanism
  exists or is planned, so the error is the better default today.

## Validation

### Baseline reproduction (before the fix, throwaway tests on merged behaviour)

A temporary `baseline_repro` test module printed the current handling of each
single-valued node, then was removed:

```
$ cargo test --bin hotcell baseline_repro -- --nocapture
BASELINE workdir = "/b"
BASELINE provisioner = Provisioner { kind: "shell", script: Some("./p.sh"), host_paths: ["/etc/machine-id"] }
BASELINE net endpoints = [Endpoint { host: "a.com", .. }, Endpoint { host: "b.com", .. }]
```

Confirms the ticket: duplicate `workdir` and `provision` were **last-wins**
(the first `host-path "/etc/hostname"` silently dropped, exit 0), while
duplicate `net` blocks **merged** their `allow` children into one allowlist
(both `a.com` and `b.com` survived) rather than replacing.

### After the fix

```
$ cargo fmt --check                                          # exit 0, clean
$ RUSTFLAGS="-D warnings" cargo clippy --all-targets --all-features
    Finished `dev` profile ...                               # no warnings
$ cargo test
    ... all 12 test binaries: test result: ok, 0 failed
```

New unit tests in `src/cellfile.rs`:
- `duplicate_workdir_is_an_error_citing_both`
- `duplicate_provision_is_an_error_citing_both`
- `duplicate_net_is_an_error_citing_both`
- `multi_valued_nodes_still_accumulate` (env/secret/host-path/allow all survive)

New integration test in `tests/run_cellfile_grammar.rs`:
- `a_duplicate_single_valued_node_fails_and_creates_no_state` — runs
  `hotcell run --dry-run`, asserts non-zero exit, the located error citing
  both occurrences, and that no `.cell` state directory is created.

Example error a user now sees (duplicate `provision`):

```
<path>/Cellfile:2:1: `provision` may be declared only once, but is declared
again here; first declared at <path>/Cellfile:1:1
```

## Findings

- Duplicate `net` behaviour (recorded per acceptance): the previous code
  merged, because `parse_net` pushed into the shared
  `decl.network.allowed_endpoints`. Endpoints from both blocks survived, so
  no `allow` child was lost — but the merge was still an unstated guess, and
  is now rejected for consistency with the fail-closed grammar.
