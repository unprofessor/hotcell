---
id: write-root-readme
kind: task
parent: polish-and-docs
title: Write the root README
status: done
assignee: null
created: 2026-07-29
updated: 2026-08-12
tags: []
depends_on: []
---

## Goal

Write the root README

## Context

Parent: `polish-and-docs`. No `README.md` at repo root today. `examples/`
and `hotcell.allium` are the best source material.

## Acceptance

- [ ] Root `README.md` covers: what hotcell is, install (`cargo install
      --path .`), the `Cellfile` format summary, `run`/`destroy`/`status`
      usage, and a pointer to `examples/pi-bootstrap`.
- [ ] Notes the network firewall status (offline vs. allowlisted egress).

## Validation

- Created `README.md` covering: what hotcell is, prerequisites (bubblewrap + Rust), install, quick start, full Cellfile key reference, all CLI commands with flags, network firewall explanation, architecture overview, and pointer to `examples/pi-bootstrap`.
- Table notes firewall status and clearly marks v1 vs post-v1 capabilities.
- `cargo build --release` → clean build.
- `cargo test --release` → 35/35 tests pass.

## Notes

- 2026-07-29 created.
- 2026-08-12 implemented.
