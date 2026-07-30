---
id: network-firewall
kind: story
parent: v1-self-hostable-hotcell
title: Network firewall
status: done
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

- 2026-07-30 all four tasks done and merged:
  - `http-connect-proxy` — hyper CONNECT allowlist proxy.
  - `wire-firewall-into-cli` — starts firewall, sets proxy env for agent.
  - `loopback-only-net` — UDS bridge through `--unshare-net`; kernel-enforced non-loopback block.
  - `firewall-tests` — integration tests + pi-bootstrap e2e (path proven with a fake key → real Google 400; real-key 200 deferred to developer).
- Open item for developer: verify `examples/pi-bootstrap` with a real API key for a 200 model response.
- Open item for tech lead: `provision.sh` seeds `~/.pi` from the host, which overrides the Cellfile's `env.PI_PROVIDER` — consider whether that should remain the default.
