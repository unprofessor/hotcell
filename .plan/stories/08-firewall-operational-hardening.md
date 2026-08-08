---
id: firewall-operational-hardening
aliases: [firewall-operational-hardening]
kind: story
parent: post-v1-firewall-policy-hardening
title: Firewall operational hardening
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [firewall, operations, reliability, phase-6]
depends_on: [audit-and-warn-tests]
---

## Goal

Bound resource use and make failure behavior deterministic before declaring the
core firewall hardening epic ready for further policy extensions.

## Context

The proxy currently has no dial timeout, tunnel idle timeout, concurrent
tunnel cap, or durable audit rotation behavior. Earlier stories establish the
decision and audit paths this work must preserve.

## Scope

- Bound dial/tunnel lifetime and concurrent tunnel count with reliable release
  on failure or cancellation.
- Bound/rotate the dedicated audit file without losing the one-record-per-
  decision contract.
- Run repeatable, non-external regression checks as the core epic's final gate.

## Notes

- [[hardening-regression-suite]] is the dependency used by the gated advanced
  epic; it must validate the entire core implementation, not merely this
  story's code.
