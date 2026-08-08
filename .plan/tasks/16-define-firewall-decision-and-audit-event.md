---
id: define-firewall-decision-and-audit-event
aliases: [define-firewall-decision-and-audit-event]
kind: task
parent: firewall-warn-audit
title: Define firewall decisions and audit event contract
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [firewall, audit, policy, phase-3]
depends_on: [document-policy-precedence-and-safety, resolved-address-integration-tests]
---

## Goal

Define a single policy-decision result and a privacy-preserving JSON audit
schema that every CONNECT outcome can produce exactly once.

## Context

The draft event mixes decision timing and tunnel lifetime. Current code has
separate early method/allow checks and late upgrade/dial failures, so an event
contract must cover every path before a writer or warn behavior is added.

## Acceptance

- [ ] A decision type distinguishes allow, ordinary policy block, sensitive
  upstream denial, unsafe-exception use, warn, malformed authority,
  unsupported method, resolution failure, and upstream failure without
  exposing internal error strings as a contract.
- [ ] A versioned JSONL schema specifies required fields including timestamp,
  cell/session identity, requested host/port when safe, resolved address when
  known, action, reason, matched rule/exception, and a precisely defined
  `duration_ms` scope.
- [ ] The contract mandates exactly one event for every incoming proxy request,
  including non-CONNECT and failures before an upstream tunnel exists.
- [ ] The contract defines redaction/privacy limits: no request body, headers,
  credentials, or tunnel payload are written.
- [ ] Warn behavior is explicitly limited to ordinary allow-rule misses;
  built-in sensitive address denials remain hard failures without the unsafe
  exception.
- [ ] Schema examples and policy outcomes are reviewable independently of a
  chosen file writer or logger framework.

## Notes

- 2026-08-08 created after error response semantics so the event contract
  reflects real client-visible paths.
