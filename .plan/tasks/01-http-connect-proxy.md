---
id: http-connect-proxy
kind: task
parent: network-firewall
title: Implement HTTP CONNECT allowlist proxy with hyper
status: review
assignee: null
created: 2026-07-29
updated: 2026-07-29
tags: []
depends_on: []
---

## Goal

Implement HTTP CONNECT allowlist proxy with hyper

## Context

Parent: `network-firewall`. Files: `src/firewall.rs` (stub), `Cargo.toml`
(`hyper`, `hyper-util`, `http-body-util`, `http`, `tokio` already present).
`NetworkPolicy` and `Endpoint` are in `src/cellfile.rs`.

## Acceptance

- [x] `firewall::start(policy)` binds a local TCP socket and runs an HTTP
      proxy (hyper) that accepts `CONNECT` requests.
- [x] A request whose destination host (and port, when specified) is in
      `NetworkPolicy::allowed_endpoints` is tunneled; everything else is
      refused (e.g. 403 / connection reset).
- [x] A policy with no allowed endpoints rejects every request.
- [x] `FirewallHandle` exposes the real listen address (not `127.0.0.1:0`).
- [x] Unit tests cover allow and deny paths.

## Notes

- 2026-07-29 created.
- 2026-07-29 implemented in `src/firewall.rs` only (no `cli.rs` /
  `isolation.rs` changes — those are separate dependent tasks).
  - `start(policy)` builds a dedicated multi-thread tokio runtime (1 worker),
    binds `127.0.0.1:0` on it via `block_on`, spawns an async hyper http1
    accept loop, and returns a `FirewallHandle` carrying the real bound
    address plus the runtime. The handle owns the runtime so the server
    lives as long as the handle; `Drop` calls `shutdown_timeout(200ms)` so
    dropping never hangs.
  - Only `CONNECT` is honored; other methods get `405 Method Not Allowed`.
  - Host matching is case-insensitive (`eq_ignore_ascii_case`); an endpoint
    with a port requires an exact port match, an endpoint with no port
    allows any port. CONNECT with no port defaults to 443 for the outbound
    dial.
  - Allowed CONNECT returns `200 OK` and spawns an upgrade task that dials
    the target and `copy_bidirectional`s bytes (raw tunnel). Denied CONNECT
    returns `403 Forbidden`.

## Validation

Ran in the worktree (`/home/exfed/projects/wt-http-connect-proxy`).

- `cargo build` — clean, no warnings.
- `cargo fmt --check` — clean.
- `cargo clippy --no-deps` — no warnings/errors.
- `cargo test firewall` — 9 passed, 0 failed:
  - `listen_addr_is_real_bound_address`: handle exposes a real
    `127.0.0.1:<port>` (not `127.0.0.1:0`) and a TCP connect to it succeeds
    → acceptance #4.
  - `allows_listed_host_port_and_tunnels`: CONNECT to an allowed
    `host:port` returns `HTTP/1.1 200` AND forwarded bytes are echoed back
    through the tunnel → acceptance #1 + #2 (allow path).
  - `allows_listed_host_with_any_port`: endpoint with no port accepts any
    port (200) → matching rule.
  - `host_match_is_case_insensitive`: policy host uppercase, CONNECT host
    lowercase → 200 → matching rule.
  - `denies_when_port_mismatches`: allowed host, wrong port → `403` → #2
    (deny path).
  - `denies_unlisted_host`: destination not in policy → `403` → #2.
  - `empty_policy_rejects_everything`: two CONNECTs against an empty policy
    both return `403` → acceptance #3.
  - `non_connect_method_is_refused`: `GET` → `405`.
  - `endpoint_allowed_unit`: direct unit checks for allow/deny, case
    insensitivity, port-any, port-mismatch, missing-port.
- `cargo test` (full suite) — all pre-existing tests still pass; no
  regressions (only `src/firewall.rs` changed).

All five acceptance criteria verified.

## Review
verdict: approved
reviewer: reviewer-1
date: 2026-07-29

Re-checked independently in the worktree `/home/exfed/projects/wt-http-connect-proxy`
(branch `plan/http-connect-proxy`). Did not trust the worker's `## Validation`;
ran every check myself and read `src/firewall.rs` against the acceptance criteria.

Commands run and results:
- `cargo build` — clean, no warnings/errors (`Finished dev profile ... 0.09s`).
- `cargo test firewall` — `test result: ok. 9 passed; 0 failed`. The 9 named
  tests match the worker's list (allow, deny, case-insensitive, port-any,
  port-mismatch, unlisted-host, empty-policy x2, non-CONNECT 405,
  endpoint_allowed_unit).
- `cargo test` (full suite) — every binary reports `0 failed`; no regressions.
- `cargo clippy --no-deps` — no warnings/errors (`Finished` with no diagnostics).
- `cargo fmt --check` — exit 0, clean.

Code assessment (src/firewall.rs, vs main diff is firewall.rs + task file only):
- #1 bind + hyper CONNECT server: `start` binds `127.0.0.1:0` on a dedicated
  multi-thread tokio runtime, spawns `serve` which runs an `http1` hyper
  accept loop with `with_upgrades()`. `handle_connect` branches on
  `Method::CONNECT` (other methods → 405). Verified by the live
  `allows_listed_host_port_and_tunnels` test issuing a real CONNECT.
- #2 allowlist tunnel + deny: `endpoint_allowed` does case-insensitive host
  match (`eq_ignore_ascii_case`) with exact port match when an endpoint
  declares a port, any-port when it does not. Allowed → 200 + spawned upgrade
  task that `copy_bidirectional`s between the upgraded client stream and a
  dialed `TcpStream` (raw tunnel — confirmed by the echo-roundtrip assertion).
  Denied → `403 Forbidden`. Port-mismatch and unlisted-host both return 403
  in the tests.
- #3 empty policy: with `allowed_endpoints: []`, `endpoint_allowed` returns
  `false` for every host/port → 403. `empty_policy_rejects_everything` tests
  two distinct destinations, both 403.
- #4 real listen address: `listen_addr` comes from `listener.local_addr()`
  (not the hardcoded placeholder). `listen_addr_is_real_bound_address`
  asserts `!= 127.0.0.1:0`, `!ends_with(":0")`, and that a TCP connect to it
  succeeds.
- #5 unit tests cover allow + deny: 9 tests covering allow path (with tunnel
  byte forwarding), deny path (unlisted, port-mismatch, empty policy),
  case-insensitivity, any-port, non-CONNECT 405, plus a direct `endpoint_allowed`
  unit test.

Lifecycle: `FirewallHandle` owns the `Runtime` via `Option<Runtime>`; `Drop`
 calls `rt.shutdown_timeout(200ms)` (bounded, won't hang). The handle is the
 only owner so the server lives exactly as long as it is held.

Diff scope: only `src/firewall.rs` and the task file changed (plus an
incidental `board.sh` line from scaffolding); no other source files touched,
consistent with the task's stated scope.

All five acceptance criteria verified by independent re-check. Approving.
