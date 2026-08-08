---
id: inject-ca-trust-into-agent-sandbox
aliases: [inject-ca-trust-into-agent-sandbox]
kind: task
parent: gated-tls-mitm-l7-policy
title: Inject per-cell CA trust into the agent sandbox
status: todo
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [firewall, tls, isolation, trust-store, phase-5]
depends_on: [implement-per-cell-ca-and-leaf-cache]
---

## Goal

Make only the approved public per-cell CA available to compatible agent
clients inside the isolated sandbox while preserving clean-environment and
no-route guarantees.

## Context

`src/isolation.rs` builds the bwrap command and currently stages only the
forwarder/bridge for networked cells. Trust injection must not leak the CA
private key, host certificate bundles, unrelated host paths, or extra network
capability into the sandbox.

## Acceptance

- [ ] The public CA is materialized/mounted or configured using the approved
  trust-store mechanism and is readable by the intended agent process only as
  needed; no private key is reachable inside the sandbox.
- [ ] `build_agent_command` keeps `--unshare-net`, clean environment behavior,
  and the existing disabled-MITM path unchanged.
- [ ] The implementation handles supported runtime trust conventions and
  clearly reports unsupported/certificate-pinning clients without silently
  falling back to interception or bypass.
- [ ] Cell-specific CA material cannot be substituted by another cell or
  persist unintentionally after the designed cleanup/rotation lifecycle.
- [ ] Tests inspect the bwrap configuration and a representative sandboxed
  process to prove public trust availability, private-key absence, and no new
  host filesystem/network exposure.
- [ ] Documentation explains operational requirements and rollback to the
  non-MITM path.

## Notes

- 2026-08-08 planned. This is the only phase-5 task expected to alter
  `src/isolation.rs`.
