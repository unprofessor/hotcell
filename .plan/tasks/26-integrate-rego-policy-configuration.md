---
id: integrate-rego-policy-configuration
aliases: [integrate-rego-policy-configuration]
kind: task
parent: gated-rego-policy
title: Integrate Rego policy configuration with the firewall
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [firewall, rego, cellfile, phase-4]
depends_on: [implement-regorus-policy-evaluator]
---

## Goal

Wire an approved Rego policy source into Cellfile/session lifecycle and the
firewall decision path without making implementation grammar a new Allium
contract.

## Context

The evaluator alone has no operator-visible policy source. The integration
must resolve any declared policy relative to a safe Cellfile-controlled
location, compile it at the approved lifecycle point, and explain its effects
through the existing audit schema.

## Acceptance

- [ ] Implement additive, documented configuration for the approved policy
  source with source-located validation and safe relative-path handling; no
  absolute or sandbox-escape path is silently accepted.
- [ ] Policy load/compile happens at the contract-defined lifecycle point and
  makes session start fail safely or stay native-only exactly as approved.
- [ ] Firewall decisions compose native and Rego results per the contract;
  Rego cannot bypass non-overridable sensitive-address checks or the explicit
  unsafe-exception rules.
- [ ] Audit records whether/which policy decision affected the result without
  logging policy source contents or request payloads.
- [ ] Existing Cellfiles with no Rego configuration behave identically to core
  policy v2 and introduce no `regorus` work on their request path.
- [ ] Integration tests cover valid configuration, missing/malformed policy,
  changed policy source, and native-only fallback/failure behavior.

## Notes

- 2026-08-08 planned. Exact dotted-key syntax stays implementation-defined.
