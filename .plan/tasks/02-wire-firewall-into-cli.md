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
