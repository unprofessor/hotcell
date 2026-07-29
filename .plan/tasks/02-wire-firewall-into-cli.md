---
id: wire-firewall-into-cli
kind: task
parent: network-firewall
title: Wire firewall into the cli run path
status: in_progress
assignee: null
created: 2026-07-29
updated: 2026-07-29
tags: []
depends_on: [http-connect-proxy]
---

## Goal

Wire firewall into the cli run path

## Context

Parent: `network-firewall`. Files: `src/cli.rs` (`run_cell`, around the
`if !provisioned.network.allowed_endpoints.is_empty()` refusal),
`src/firewall.rs`, `src/isolation.rs` (`build_agent_command`).

## Acceptance

- [ ] `run_cell` starts the firewall for the provisioned network policy and
      gets its listen address.
- [ ] The agent's env includes `HTTP_PROXY`/`HTTPS_PROXY` pointing at the
      proxy and a `NO_PROXY` that keeps loopback local.
- [ ] The hard refusal for non-empty `net.allow` is removed; an empty policy
      stays fully offline (no proxy started, `--unshare-net` kept).
- [ ] Existing CLI tests still pass.

## Notes

- 2026-07-29 created. Depends on `http-connect-proxy`.
