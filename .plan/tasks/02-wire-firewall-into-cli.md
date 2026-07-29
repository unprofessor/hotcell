---
id: wire-firewall-into-cli
kind: task
parent: network-firewall
title: Wire firewall into the cli run path
status: review
assignee: null
created: 2026-07-29
updated: 2026-07-29
tags: []
depends_on: [http-connect-proxy]
---

## Goal

Wire firewall into the cli run path

## Context

Parent: `network-firewall`. Files: `src/cli.rs` (`run_cell`, around the
`if !provisioned.network.allowed_endpoints.is_empty()` refusal),
`src/firewall.rs`, `src/isolation.rs` (`build_agent_command`).

## Acceptance

- [x] `run_cell` starts the firewall for the provisioned network policy and
      gets its listen address.
- [x] The agent's env includes `HTTP_PROXY`/`HTTPS_PROXY` pointing at the
      proxy and a `NO_PROXY` that keeps loopback local.
- [x] The hard refusal for non-empty `net.allow` is removed; an empty policy
      stays fully offline (no proxy started, `--unshare-net` kept).
- [x] Existing CLI tests still pass.

## Notes

- 2026-07-29 created. Depends on `http-connect-proxy`.
- 2026-07-29 implemented in `src/cli.rs` `run_cell`. Removed the hard refusal
  block; when `provisioned.network.allowed_endpoints` is non-empty, call
  `crate::firewall::start(&provisioned.network)`, push `HTTP_PROXY`/
  `HTTPS_PROXY` = `http://<listen_addr>` and `NO_PROXY` = `127.0.0.1,localhost`
  onto the agent env, and hold the `FirewallHandle` for the lifetime of the
  agent child (explicit `drop(_firewall)` after `child.wait()`, since the
  function exits via `std::process::exit` which skips `Drop`). Empty policy
  starts no firewall and keeps the existing `--unshare-net` offline behavior
  (added unconditionally by `build_agent_command`).
- 2026-07-29 scope note: actually routing the agent's traffic to the host
  loopback proxy through a relaxed network namespace is the separate
  `loopback-only-net` task (depends on this one). This task only wires the
  start + env; `build_agent_command` still always passes `--unshare-net`, so
  reachability of the proxy from inside the sandbox is out of scope here.

## Validation

Ran in the worktree `/home/exfed/projects/wt-wire-firewall-into-cli`:

- `cargo build` — clean, no warnings/errors.
- `cargo test` — all suites green. Per-suite results:
  - lib (incl. `firewall::tests::*`, 9 tests): 13 passed
  - `tests/run_cell_override.rs`: 3 passed
  - `tests/run_file_override.rs`: 4 passed
  - `tests/run_global_flags.rs`: 4 passed
  - `tests/run_minimal.rs`: 2 passed
  - `tests/run_risk_profiles.rs`: 4 passed
  - `tests/run_script_provisioner.rs`: 4 passed
  - `cargo test firewall` filter: 9 firewall unit tests pass
- Acceptance 1 (run_cell starts firewall + gets listen addr) and 2 (env has
  proxy vars): manual smoke test with a Cellfile `net.allow = api.openai.com:443`,
  `hotcell run /usr/bin/printenv HTTP_PROXY HTTPS_PROXY NO_PROXY`:
  stderr printed `network firewall: HTTP allowlist proxy on 127.0.0.1:<port> (1 endpoint(s))`
  and stdout printed the three values:
  `http://127.0.0.1:<port>` (HTTP_PROXY), `http://127.0.0.1:<port>` (HTTPS_PROXY),
  `127.0.0.1,localhost` (NO_PROXY). Exit 0.
- Acceptance 3 (refusal removed; empty policy offline): manual smoke test with
  an empty Cellfile, `hotcell run /usr/bin/printenv HTTP_PROXY HTTPS_PROXY NO_PROXY`
  produced no `network firewall:` line (no proxy started) and printenv found
  none of the proxy vars set (exit 1, empty stdout). `--unshare-net` is still
  added unconditionally by `build_agent_command`, so the empty-policy path
  remains fully offline.

## Review
verdict: approved
reviewer: reviewer-1
date: 2026-07-29

Re-checked everything independently in the worktree; did not rely on the
worker's self-validation.

Code review (`git diff main..plan/wire-firewall-into-cli`, `src/cli.rs`,
`src/firewall.rs`, `src/isolation.rs`):
- The hard refusal block (`if !allowed_endpoints.is_empty() { ... exit(1) }`)
  is removed. Confirmed.
- Firewall is started only when `provisioned.network.allowed_endpoints` is
  non-empty, guarded by `if !provisioned.network.allowed_endpoints.is_empty()`.
  Empty policy falls into the `else => None` branch: no `firewall::start`, no
  proxy env. Confirmed.
- `crate::firewall::start(&provisioned.network)` matches the actual API in
  `src/firewall.rs`: `pub fn start(policy: &NetworkPolicy) ->
  anyhow::Result<FirewallHandle>`. `handle.listen_addr()` matches
  `pub fn listen_addr(&self) -> &str`. Confirmed.
- `HTTP_PROXY` and `HTTPS_PROXY` are both set to `http://<listen_addr>`;
  `NO_PROXY` = `127.0.0.1,localhost`. Pushed onto `env` only inside the
  non-empty branch. Confirmed.
- `FirewallHandle` is held as `let _firewall: Option<...>` across
  `cmd.spawn()` / `child.wait()`, then explicitly `drop(_firewall)` after the
  child exits (necessary because the fn returns via `std::process::exit`,
  which skips Drop). Confirmed.
- `src/isolation.rs` `build_agent_command` still always passes `--unshare-net`
  (line 207), so the empty-policy path stays fully offline and (per the scope
  note) the agent cannot yet reach the host-loopback proxy — out of scope for
  this task. Confirmed.

Commands run in the worktree:
- `cargo build` — clean, no warnings/errors.
- `cargo test` — all green. Per-suite: lib 13 passed; run_cell_override 3;
  run_file_override 4; run_global_flags 4; run_minimal 2; run_risk_profiles 4;
  run_script_provisioner 4. Total 34 passed, 0 failed.
- `cargo clippy --no-deps` — clean, no warnings.
- `cargo fmt --check` — clean (exit 0).

Manual smoke tests (reproduced myself with the built binary):
- Non-empty policy: Cellfile `net.allow = api.openai.com:443`, `hotcell run
  /usr/bin/printenv HTTP_PROXY HTTPS_PROXY NO_PROXY`. stderr printed
  `network firewall: HTTP allowlist proxy on 127.0.0.1:35759 (1 endpoint(s))`;
  stdout printed `http://127.0.0.1:35759` (HTTP_PROXY),
  `http://127.0.0.1:35759` (HTTPS_PROXY), `127.0.0.1,localhost` (NO_PROXY);
  exit 0. Acceptance 1 & 2 met.
- Empty policy: empty Cellfile, same command. No `network firewall:` line;
  printenv printed nothing; exit 1 (expected — env vars unset). Acceptance 3
  met (refusal gone, stays offline, no proxy).

All four acceptance criteria satisfied. Out-of-scope item (actual traffic
routing to the proxy via relaxed net namespace) is correctly deferred to the
`loopback-only-net` task and was not judged here.
