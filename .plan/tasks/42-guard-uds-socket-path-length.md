---
id: guard-uds-socket-path-length
aliases: [guard-uds-socket-path-length]
kind: task
parent: firewall-operational-hardening
title: Guard the UDS bridge socket path against SUN_LEN
status: todo
assignee: null
created: 2026-08-24
updated: 2026-08-24
tags: [firewall, reliability, bridge]
depends_on: []
---

## Goal

Stop the firewall from failing at startup when the UDS bridge socket path is
too long for `sockaddr_un`.

## Context

`FirewallHandle::start_uds_bridge` binds a Unix-domain socket at `uds_path`
(`src/firewall.rs:79`, `UnixListener::bind(uds_path)`) with no length check.
`sockaddr_un.sun_path` is bounded by `SUN_LEN` (108 bytes on Linux, including
the NUL), so a long enough path makes `bind` fail with an OS error before the
proxy is usable. This was hit in practice during a `pi-bootstrap` run whose
scratchpad-derived socket directory was deep enough to exceed the limit — the
firewall never came up.

This is a **present-day bug in the current bridge**, not part of the policy-v2
work. It is filed under this story because the story owns proxy reliability and
deterministic failure behaviour, but it carries no `depends_on` and does not
wait on the v2 chain — it can and should be fixed independently, and the fix
should survive into v2 unchanged.

## Approach

Sketches, none chosen — the task owner decides:

1. **Fail early with a clear error.** Check the path length before `bind` and
   return an actionable error naming the path and the limit, instead of an
   opaque OS error mid-startup. Cheapest; turns a confusing failure into a
   legible one but does not make long paths work.
2. **Shorten the path.** Derive the socket location from a short, stable base
   (e.g. a short per-cell dir under a shorter root, or a hashed short name)
   rather than nesting under a long scratchpad path, so realistic invocations
   stay well under `SUN_LEN`. Requires care: the directory must still be
   bind-mounted into both sides of the bridge (see the `start_uds_bridge` doc
   comment) and cleaned up.
3. **Both.** Shorten by default and keep the length guard as a hard backstop
   with a clear message.

Whatever is chosen, the socket must remain in a directory reachable from both
the host relay and the in-namespace forwarder, and stale-socket removal
(`remove_file` before `bind`) must be preserved.

## Acceptance

- [ ] A socket path that would exceed `SUN_LEN` no longer produces an opaque
  bind failure: either it is avoided by construction, or it fails with a clear
  error that names the offending path and the limit.
- [ ] A realistic deep scratchpad/cell path (reproducing the observed
  `pi-bootstrap` failure) brings the firewall up successfully, or the chosen
  design documents why a hard limit is unavoidable and surfaces it cleanly.
- [ ] The bridge still works end to end: the in-namespace forwarder reaches
  the host relay, stale sockets are still cleared, and cleanup is unchanged.
- [ ] A regression test covers the long-path case (unit test on the
  length/derivation logic is sufficient; a full bridge integration test is not
  required if the derivation is unit-testable).
- [ ] `cargo fmt --check`, `cargo clippy`, and `cargo test` are clean.

## Notes

- 2026-08-24 created by the leader from a finding surfaced during the
  `document-kdl-cellfile-format` review, which hit the failure on a long
  scratchpad socket path. Placed under [[firewall-operational-hardening]] as a
  reliability item; deliberately left dependency-free so it is not gated behind
  the policy-v2 chain.
