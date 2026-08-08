---
id: hardening-regression-suite
aliases: [hardening-regression-suite]
kind: task
parent: firewall-operational-hardening
title: Run the firewall hardening regression suite
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [firewall, regression, verification, phase-6]
depends_on: [resolved-address-integration-tests, bound-audit-lifecycle]
---

## Goal

Establish a repeatable, non-external evidence gate that the complete core
firewall hardening epic preserves safety, compatibility, and operational
behavior before advanced extensions can be considered.

## Context

This is the final core task and the dependency gate for the advanced Rego and
TLS stories. Earlier work changes policy representation, CLI activation,
CONNECT timing, audit I/O, warn behavior, and resource limits; no single
narrow unit suite proves their interaction.

## Acceptance

- [ ] Add or organize a deterministic regression command/documented test set
  that covers legacy Cellfiles, empty-policy offline isolation, CIDR-only
  bridge activation, sensitive-address denial, explicit unsafe exceptions,
  resolve-once dialing, warn mode, audit JSONL, and resource bounds.
- [ ] Re-run local network/namespace-sensitive tests enough times to detect
  deterministic regressions; record the repetition and result in validation.
- [ ] `cargo fmt --check`, `cargo clippy -- -D warnings`, and `cargo test`
  succeed from a clean worktree without provider credentials or public egress.
- [ ] The final evidence confirms no agent network route was introduced and
  no test fixture relaxed production defaults.
- [ ] Any intentionally deferred concern is recorded with an owner/follow-up
  rather than silently treated as passing.

## Notes

- 2026-08-08 created as the core epic's release gate and the only permitted
  prerequisite for unblocking advanced policy gate decisions.
