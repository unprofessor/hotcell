---
id: git-bridge-transport
kind: story
parent: post-v1-workspace-sync
title: Git transport over the UDS bridge
status: todo
assignee: null
created: 2026-09-06
updated: 2026-09-06
tags: [sync, bridge, transport, git]
depends_on: [workspace-repo-model, guard-uds-socket-path-length]
---

## Goal

Carry git's pack protocol from inside the cell to the host-side shadow repos,
so the agent sees an unremarkable `origin`.

## Context

The hard parts already exist for the network firewall: a read-only-bound UDS
directory the agent can connect to but not replace (`src/isolation.rs:88-92`),
and an in-namespace forwarder that is hotcell re-invoked as a subcommand with
its binary bound read-only (`src/isolation.rs:82-86`). This story is largely
that pattern applied to git.

## Scope

- One socket for all repos, with the repo name carried in the protocol — not
  one socket per repo. Keeps the path fixed and known-good (see
  [[guard-uds-socket-path-length]]) and gives one site for a future
  allow/deny/scan decision with the repo as an audit field.
- Site it in `BRIDGE_DIR` (`/hotcell-bridge`), not under `/dev` — `/dev` is a
  fresh bwrap devtmpfs (`src/isolation.rs:211`).
- `git-remote-hotcell` helper (or `ext::`) rather than a `git://` loopback
  hop: no port allocation, discovery, or collision. Bind the hotcell binary
  read-only under a second name and dispatch on `argv[0]`.
- Host-side routing from repo name to shadow bare repo.
- Render the whole thing in the execution plan — the plan is the trust
  artifact (`src/plan.rs:16`).

## Acceptance

- [ ] `git fetch`/`push` against `origin` works inside the cell with no
  hotcell-specific configuration visible to the agent.
- [ ] One socket serves N repos; an unknown repo name fails cleanly.
- [ ] The socket cannot be replaced or written from inside the cell.
- [ ] The transport appears in `--dry-run` plan output accurately.
- [ ] Works with the agent profile's `--unshare-net` intact.
- [ ] `cargo fmt --check`, `cargo clippy`, `cargo test` clean.

## Notes

- 2026-09-06 filed from the settled design in [[post-v1-workspace-sync]].
