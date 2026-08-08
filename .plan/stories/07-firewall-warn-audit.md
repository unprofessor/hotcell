---
id: firewall-warn-audit
aliases: [firewall-warn-audit]
kind: story
parent: post-v1-firewall-policy-hardening
title: Firewall warn mode and audit
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [firewall, audit, observability, phase-3]
depends_on: [return-correct-connect-failures]
---

## Goal

Provide a rollout-safe warn mode and a durable, structured decision trail for
every CONNECT request without exposing traffic contents or weakening
sensitive-address protection.

## Context

`state::log_file_path` only derives a `session.log` path today; the CLI prints
that path but does not install a writer. Audit therefore needs explicit
ownership, synchronization, creation, failure behavior, and session lifecycle
rather than assuming an existing log sink. The decision contract must clarify
whether duration measures authorization or tunnel lifetime.

## Scope

- Define a single JSON audit event and decision taxonomy.
- Create a session-scoped JSONL sink owned by the host-side firewall.
- Permit only ordinary allow-rule misses in `warn` mode; sensitive addresses
  still require the dedicated unsafe exception.

## Notes

- The implementation and verification chain is
  [[define-firewall-decision-and-audit-event]] →
  [[implement-session-scoped-jsonl-audit]] →
  [[implement-firewall-warn-mode]] → [[audit-and-warn-tests]].
