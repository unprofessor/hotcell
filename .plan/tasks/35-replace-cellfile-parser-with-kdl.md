---
id: replace-cellfile-parser-with-kdl
aliases: [replace-cellfile-parser-with-kdl]
kind: task
parent: cellfile-kdl-grammar-and-secrets
title: Replace the Cellfile parser with KDL v2
status: in_progress
assignee: worker-1
created: 2026-08-23
updated: 2026-08-23
tags: [cellfile, grammar, parser]
depends_on: []
---

## Goal

Parse Cellfiles as KDL v2 documents, carry the new env source model in
`CellDeclaration`, and migrate every Cellfile in the repo. Must land green:
the tests cannot compile against the old format, so their migration belongs
here.

## Context

`src/cellfile.rs` hand-rolls a line format in ~200 lines (`parse`,
`split_kv`, `strip_comment`, `parse_endpoint`, `parse_file_mapping`). Replace
that with a walk over `kdl::KdlDocument`. Grammar and rationale are fixed in
the parent story — do not re-litigate them.

The env model changes shape, so `provisioned_as.json` records written by older
versions will not load. That is accepted; no migration path is required for
cell state.

Cellfiles to migrate: `examples/pi-bootstrap/Cellfile`, plus inline Cellfiles
written by 9 integration tests (`tests/run_*.rs`, `tests/destroy_*.rs`).

## Acceptance

- [ ] `kdl` 6.x is a dependency; `Cargo.toml` declares `rust-version = "1.95"`
  and CI still passes on stable and beta.
- [ ] `CellDeclaration.environment` holds `EnvVar { key, source }` where
  `source` is `Literal(String) | Passthrough | Secret`. No resolved value for
  a passthrough or secret is ever stored in the declaration.
- [ ] `env NAME="value"` parses as `Literal`, `env NAME` as `Passthrough`,
  `secret NAME` as `Secret`. A node carrying both an argument and a property
  is a source-located error.
- [ ] `net { allow "host" port=443 }` parses; `port` is optional and absent
  means any port. IPv6 literals parse correctly — the `rsplit_once(':')` bug
  is gone, with a regression test.
- [ ] `provision "shell" script="..."` with `host-path` children parses;
  `provision "none"` and an empty document both yield the documented defaults
  (no-op provisioner, offline, workdir `/work`).
- [ ] `package` and `seed` are not in the grammar. Encountering either yields
  an error naming the node and stating that it is not implemented.
- [ ] Unknown nodes remain errors (fail-closed, as today).
- [ ] Parse errors carry `path:line:col` and are `anyhow` values — kdl's
  miette diagnostic is flattened, not surfaced, and `miette`'s fancy feature
  is not added.
- [ ] A file that still uses the old `key = value` shape produces a migration
  hint rather than a bare KDL syntax error.
- [ ] `examples/pi-bootstrap/Cellfile` and all test Cellfiles are migrated;
  `cargo test` and `cargo clippy --all-targets` pass.

## Notes

- 2026-08-23 created. Verified during design: KDL v2 accepts bare identifier
  arguments, and args-vs-props cleanly separates the two `env` shapes.
- Resolution of passthrough values is NOT in this task — the parser only
  records which vars are passthrough or secret. See
  `resolve-env-passthrough-and-secrets`.
- 2026-08-23 developer confirmed: **breaking compatibility and eliding
  migration are both fine** — hotcell is still internal. No compat shim, no
  state migration, no dual-format parsing. The old-format hint in the
  acceptance criteria stays only because it is a few lines of error handling
  that the team will hit with its own existing Cellfiles; it is not a
  compatibility path.
