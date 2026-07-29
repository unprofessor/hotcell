---
id: firewall-tests
kind: task
parent: network-firewall
title: Firewall tests and a pi-bootstrap run against a provider
status: todo
assignee: null
created: 2026-07-29
updated: 2026-07-29
tags: []
depends_on: [http-connect-proxy, wire-firewall-into-cli, loopback-only-net]
---

## Goal

Firewall tests and a pi-bootstrap run against a provider

## Context

Parent: `network-firewall`. Files: `tests/` (new integration test),
`examples/pi-bootstrap/Cellfile` (has commented-out `net.allow` lines for
Gemini/Anthropic/OpenAI).

## Acceptance

- [ ] Integration test: a cell with an allowlisted endpoint can reach it via
      the proxy; a non-allowlisted endpoint is blocked.
- [ ] `examples/pi-bootstrap` runs `hotcell run -- pi` against a real provider
      end-to-end (manually verified with an API key).
- [ ] `cargo test --release` is green.

## Notes

- 2026-07-29 created. Depends on the other three firewall tasks.
