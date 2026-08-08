---
id: gated-rego-policy
aliases: [gated-rego-policy]
kind: story
parent: post-v1-advanced-firewall-policy
title: Gated Rego policy extension
status: blocked
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [firewall, rego, gated, phase-4]
depends_on: [hardening-regression-suite]
---

## Goal

Offer an optional, fail-closed Rego extension only if real policy composition
needs exceed the native policy v2 model.

## Gate

Keep this story blocked until a concrete shared/multi-cell policy use case,
performance budget, failure-mode requirements, and approval to add `regorus`
are documented in [[approve-rego-policy-gate]]. A generic desire for
flexibility is not sufficient justification.

## Context

The core epic intentionally uses a native matcher. If approved, Rego consumes
stable firewall decision input and must compile/cache policies per cell rather
than compile or load them on every request. The Allium model may describe an
optional evaluator/decision guarantee, but must not prescribe a `net.policy`
Cellfile key.

## Notes

- Implementation is sequenced entirely after the gate: contract, evaluator,
  configuration wiring, then fail-closed integration tests.
