---
id: return-correct-connect-failures
aliases: [return-correct-connect-failures]
kind: task
parent: firewall-resolved-address-enforcement
title: Return deterministic CONNECT policy and upstream failures
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [firewall, http, errors, phase-2]
depends_on: [resolve-filter-and-dial-once]
---

## Goal

Make policy and upstream failures visible to a CONNECT client before Hotcell
returns a successful tunnel response, rather than acknowledging `200` and
silently closing the upgraded stream later.

## Context

Today `handle_connect` returns `200 OK` before its spawned task connects to the
upstream. Filtering inside that task cannot report `403`, and an upstream
connect failure is only a best-effort client shutdown.

## Acceptance

- [ ] Authority parsing, resolve/filter, and the required preflight dial occur
  early enough that a denied target returns the documented policy status and
  an allowed-but-unreachable target returns a deterministic proxy/upstream
  failure status before `200` is emitted.
- [ ] Non-CONNECT and malformed authority requests retain explicit,
  documented client-visible statuses and cannot reach resolver/dial code.
- [ ] A `200` CONNECT result means an authorized upstream connection is ready
  for the upgrade/tunnel path, aside from later tunnel lifecycle failures.
- [ ] Error paths close resources safely, preserve no-route isolation, and
  create a decision result usable by later audit work.
- [ ] Focused tests prove that denied, unresolved, refused, and permitted
  targets produce their intended status without external network access.

## Notes

- 2026-08-08 created to restructure response timing, not merely replace one
  `TcpStream::connect` call.
