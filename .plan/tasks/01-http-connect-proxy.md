---
id: http-connect-proxy
kind: task
parent: network-firewall
title: Implement HTTP CONNECT allowlist proxy with hyper
status: in_progress
assignee: null
created: 2026-07-29
updated: 2026-07-29
tags: []
depends_on: []
---

## Goal

Implement HTTP CONNECT allowlist proxy with hyper

## Context

Parent: `network-firewall`. Files: `src/firewall.rs` (stub), `Cargo.toml`
(`hyper`, `hyper-util`, `http-body-util`, `http`, `tokio` already present).
`NetworkPolicy` and `Endpoint` are in `src/cellfile.rs`.

## Acceptance

- [ ] `firewall::start(policy)` binds a local TCP socket and runs an HTTP
      proxy (hyper) that accepts `CONNECT` requests.
- [ ] A request whose destination host (and port, when specified) is in
      `NetworkPolicy::allowed_endpoints` is tunneled; everything else is
      refused (e.g. 403 / connection reset).
- [ ] A policy with no allowed endpoints rejects every request.
- [ ] `FirewallHandle` exposes the real listen address (not `127.0.0.1:0`).
- [ ] Unit tests cover allow and deny paths.

## Notes

- 2026-07-29 created.
