---
id: network-firewall
kind: story
parent: v1-self-hostable-hotcell
title: Network firewall
status: todo
assignee: null
created: 2026-07-29
updated: 2026-07-29
tags: []
---

## Goal

Network firewall

## Context

Parent: `v1-self-hostable-hotcell`. The `NetworkFirewall` guarantee in
`hotcell.allium` requires that the agent can only reach endpoints in the
cell's provisioned `network.allowed_endpoints`; enforcement is via an HTTP
proxy, non-HTTP traffic blocked by default, and a cell with no allowed
endpoints has no network access.

Today:
- `src/firewall.rs` is a stub (`start()` returns `127.0.0.1:0`, no server).
- `src/cli.rs` refuses to run a cell with non-empty `network.allowed_endpoints`.
- `src/isolation.rs` `build_agent_command` uses `--unshare-net` (fully offline).
- `hyper` is already in `Cargo.toml`.
- `hotcell.allium` open question (line ~512): hostname/domain allowlist is
  sufficient for v1.

## Notes

- 2026-07-29 created. Tasks:
  - `http-connect-proxy` — the proxy server.
  - `wire-firewall-into-cli` — start it on `run`, set proxy env for the agent.
  - `loopback-only-net` — allow loopback egress so the agent can reach the
    proxy while staying blocked from everything else.
  - `firewall-tests` — tests + a real `examples/pi-bootstrap` run.
