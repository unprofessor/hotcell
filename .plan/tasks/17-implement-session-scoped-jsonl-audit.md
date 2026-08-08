---
id: implement-session-scoped-jsonl-audit
aliases: [implement-session-scoped-jsonl-audit]
kind: task
parent: firewall-warn-audit
title: Implement session-scoped JSONL firewall audit
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [firewall, audit, jsonl, phase-3]
depends_on: [define-firewall-decision-and-audit-event]
---

## Goal

Create a host-owned, synchronized session audit sink and connect it to the
firewall decision path without pretending that the existing path-only
`session.log` is already a usable writer.

## Context

`state::log_file_path` currently just constructs a path and `run_cell` prints
it. The firewall runs tasks concurrently on its own Tokio runtime, so event
writes need explicit ownership and synchronization. The audit log must live
outside the agent-visible rootfs.

## Acceptance

- [ ] Starting a network-enabled session creates/opens a documented dedicated
  session-scoped firewall JSONL destination under host-owned cell state; its
  relationship to `session.log` is explicit rather than assumed.
- [ ] Audit sink ownership/configuration is passed into `firewall::start` or an
  equally testable host-side boundary, not through the UDS forwarder or agent
  environment.
- [ ] Concurrent request handling cannot interleave JSON lines; each successful
  write is one parseable event matching the approved schema.
- [ ] Directory creation, file open/write failure, flush/close behavior, and
  whether an audit-sink failure fails closed or disables egress are documented
  and tested.
- [ ] The sink never records tunnel payloads, headers, credentials, or agent
  filesystem data.
- [ ] Unit/integration tests prove audit emission on allow, policy block,
  upstream denial, malformed request, and upstream failure paths.

## Notes

- 2026-08-08 created. Log size/rotation is intentionally deferred to
  `bound-audit-lifecycle`.
