---
id: test-rego-policy-integration
aliases: [test-rego-policy-integration]
kind: task
parent: gated-rego-policy
title: Test Rego policy integration and fail-closed behavior
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [firewall, rego, tests, phase-4]
depends_on: [integrate-rego-policy-configuration]
---

## Goal

Independently prove the approved Rego extension is bounded, privacy-preserving,
and cannot make the core firewall less safe.

## Context

This is the final validation task for a gated policy engine. It must exercise
the full configuration-to-proxy path, not only evaluator unit functions.

## Acceptance

- [ ] Deterministic tests cover default deny, approved allow, explicit deny,
  malformed/missing policy, compilation/evaluation errors, and cache reuse.
- [ ] Tests prove native sensitive-address denials and no-route isolation
  survive a policy that tries to allow broader traffic.
- [ ] Tests verify configuration path validation and that a Cellfile without
  Rego configuration retains exact core-v2 behavior.
- [ ] Audit tests show a privacy-safe policy identifier/reason but no policy
  body, request headers, credentials, or tunnel payload.
- [ ] Resource/performance checks demonstrate compilation is not per request
  and that the approved budget/failure behavior is enforced.
- [ ] `cargo fmt --check`, `cargo clippy -- -D warnings`, and the full test
  suite pass from a clean worktree.

## Notes

- 2026-08-08 planned. A reviewer must re-run this evidence before the gated
  story can be closed.
