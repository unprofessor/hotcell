---
id: post-v1-workspace-sync
kind: epic
title: Post-V1 workspace sync between host and cell
status: todo
assignee: null
created: 2026-09-05
updated: 2026-09-05
tags: [sync, vcs, workspace, post-v1]
---

## Goal

Give a cell a way to exchange source with the host across the sandbox
boundary. Today there is none: the agent's only filesystem is the cell rootfs
(`.cell/<name>/fs`), all host binds are read-only, and the agent gets none of
them (`src/isolation.rs:8-18`). Getting work out means the developer reaching
into `.cell/<name>/fs/` by hand.

## Scope

- Brokered VCS sync via a host-side shadow remote — story `brokered-vcs-sync`.

## Out of scope

- Egress to real remotes (GitHub et al). That is the existing CONNECT proxy
  plus per-cell scoped credentials, and is an orthogonal feature — a cell can
  reasonably use both. Owned by [[post-v1-firewall-policy-hardening]].
- Dependency ingress (cargo, npm). Also the CONNECT proxy's job; distinct
  trust properties from workspace exchange and should not be conflated.

## Notes

- 2026-09-05 created to home workspace sync, which fits none of the existing
  epics. Strawman only — see [[brokered-vcs-sync]] for what is settled and
  what is open.
