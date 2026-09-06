---
id: git-bridge-transport
kind: story
parent: workspace-sync
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

The *pattern* exists for the network firewall — a read-only-bound UDS directory
the agent can connect to but not replace (`src/isolation.rs:88-92`), and an
in-namespace forwarder that is hotcell re-invoked as a subcommand with its
binary bound read-only (`src/isolation.rs:82-86`). The *plumbing* does not:

- The firewall's socket is `proxy.sock` (`src/cli.rs:271`) and carries HTTP
  CONNECT. Git's pack protocol cannot share it. This story adds a second
  socket alongside it.
- The bridge directory, bind, and forwarder are created only when
  `plan.bridge` is `Some` (`src/cli.rs:259-278`), which requires a non-empty
  network policy — "Empty policy (offline): no firewall, no bridge"
  (`src/cli.rs:240-242`). An offline cell that wants sync gets none of it
  today.

So this story also generalizes `plan.bridge` from a firewall concept to a cell
bridge that either feature can populate.

## Scope

- Generalize the bridge so it stands up with no network policy present, while
  leaving the airtight offline path intact: a synced cell with an empty
  allowlist still reaches nothing but its shadow.
- One git socket for all repos, with the repo name carried in the protocol —
  not one socket per repo, and not the firewall's socket. Keeps the path fixed
  and known-good (see [[guard-uds-socket-path-length]]) and gives one site for
  a future allow/deny/scan decision with the repo as an audit field.
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
- [ ] One git socket serves N repos; an unknown repo name fails cleanly.
- [ ] The socket cannot be replaced or written from inside the cell.
- [ ] The transport appears in `--dry-run` plan output accurately.
- [ ] Works with the agent profile's `--unshare-net` intact.
- [ ] Sync works in a cell with an **empty** network policy, and that cell
  still has no egress — the offline path stays airtight.
- [ ] Sync and the firewall coexist in one cell, each with its own socket.
- [ ] `cargo fmt --check`, `cargo clippy`, `cargo test` clean.

## Notes

- 2026-09-06 filed from the settled design in [[workspace-sync]].
