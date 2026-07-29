---
id: loopback-only-net
kind: task
parent: network-firewall
title: Relax agent isolation to loopback-only plus proxy
status: todo
assignee: null
created: 2026-07-29
updated: 2026-07-29
tags: []
---

## Goal

Relax agent isolation to loopback-only plus proxy

## Context

Parent: `network-firewall`. File: `src/isolation.rs` (`build_agent_command`,
currently `--unshare-net`). The proxy listens on loopback; the agent must reach
it but nothing else. `--unshare-net` blocks even loopback to the host proxy,
so a network policy cannot be enforced while it's set.

## Acceptance

- [ ] Agent profile allows loopback egress to the proxy's port only (e.g.
      bwrap without `--unshare-net` plus a loopback-only setup, or a net
      namespace with a loopback route to the proxy).
- [ ] Non-loopback egress remains blocked for the agent.
- [ ] An offline cell (empty policy) still has no network at all.
- [ ] Risk-profile isolation tests in `tests/run_risk_profiles.rs` still pass.

## Notes

- 2026-07-29 created. Depends on `wire-firewall-into-cli`; coordinate with it
  since both touch the agent launch path.
