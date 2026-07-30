---
id: firewall-tests
kind: task
parent: network-firewall
title: Firewall tests and a pi-bootstrap run against a provider
status: review
assignee: null
created: 2026-07-29
updated: 2026-07-31
tags: []
depends_on: [http-connect-proxy, wire-firewall-into-cli, loopback-only-net]
---

## Goal

Firewall tests and a pi-bootstrap run against a provider

## Context

Parent: `network-firewall`. Files: `tests/` (new integration test),
`examples/pi-bootstrap/Cellfile` (has commented-out `net.allow` lines for
Gemini/Anthropic/OpenAI).

## Acceptance

- [x] Integration test: a cell with an allowlisted endpoint can reach it via
      the proxy; a non-allowlisted endpoint is blocked.
- [ ] `examples/pi-bootstrap` runs `hotcell run -- pi` against a real provider
      end-to-end (manually verified with an API key).
      — PATH PROVEN end-to-end through pi (real Google `API_KEY_INVALID` 400
      response via firewall proxy -> CONNECT 200 -> TLS), but a REAL API key
      was unavailable, so a 200 model response was NOT observed. See
      `## Validation` Acceptance #2; flagged for the developer.
- [x] `cargo test --release` is green.

## Notes

- 2026-07-29 created. Depends on the other three firewall tasks.
- 2026-07-30 RE-DISPATCHED after an interrupt. Tech lead diagnosed a
  deterministic pre-existing failure in `networked_agent_reaches_loopback_proxy`
  (tests/run_loopback_network.rs): the echo server's two `write_all` calls
  arrived as separate TCP segments and the single `s.recv(128)` got only
  `b"echo:"`. Fix folded into this task (acceptance #3 needs the full release
  suite green).

## Validation

Re-ran every check myself (commands + observed results). No real API key was
available or committed; the pi-bootstrap provider verification used a
deliberately fake key to provoke a network-level provider response.

### Acceptance #1 — integration test (allow + deny through the proxy)
File: `tests/run_firewall_integration.rs` (committed in 46ff3ed).
Test `allowlisted_reachable_and_non_allowlisted_blocked` runs a single cell
with two host-loopback echo servers on distinct OS-assigned ports; only one
port is in `net.allow`. The in-cell python agent reads `HTTP_PROXY` (set by
the in-namespace forwarder) and issues `CONNECT` to both through the bridge:
- allowlisted endpoint -> `HTTP/1.1 200` + echo round-trip `echo:hotcell-e2e`
  through the tunnel (accumulates bytes across segmented writes — same fix as
  the loopback test).
- non-allowlisted endpoint (same host, different port) -> proxy itself returns
  `HTTP/1.1 403` (policy deny inside the firewall, distinct from the kernel
  non-loopback block covered in `run_loopback_network.rs`).
Command: `cargo test --release --test run_firewall_integration`
Result: `1 passed; 0 failed` — confirmed across 3 consecutive runs (not flaky).

### Acceptance #2 — pi-bootstrap run against a real provider (end-to-end)
Honest, observed behavior. No real API key was available, so I verified the
network/firewall path works end-to-end and flagged what remains for the
developer.

Setup: copied `examples/pi-bootstrap` to `/tmp/pi-boot-test`, uncommented
`net.allow = generativelanguage.googleapis.com:443`, set a FAKE
`env.GOOGLE_API_KEY` (never committed).

Observations:
1. `hotcell run -- pi -p "..." --no-tools` -> exit 1, `Connection error.`
   JSON mode revealed pi used `provider: opencode, model: glm-5.2` — NOT
   google. Root cause: `provision.sh` seeds `~/.pi` from the host, and the
   seeded host config (opencode) OVERRIDES the Cellfile's `env.PI_PROVIDER`.
   pi then targets the opencode endpoint, which is not in `net.allow`.
2. `--provider google --model gemini-2.5-flash` (no key arg) -> exit 1,
   `No API key found for google.` (seeded config suppresses env key
   resolution).
3. `--provider google --model gemini-2.5-flash --api-key <fake>` -> exit 0
   with a REAL Google API response through the firewall:
   `provider: google, api: google-generative-ai`, errorMessage =
   `{"code":400,"message":"API key not valid. Please pass a valid API key.",
     "status":"INVALID_ARGUMENT","reason":"API_KEY_INVALID",
     "domain":"googleapis.com"}`.
   This proves the full egress path: pi -> `HTTPS_PROXY` -> firewall proxy ->
   `CONNECT 200` -> TLS handshake (real Google cert, TLS_AES_256_GCM_SHA384)
   -> `generativelanguage.googleapis.com` -> real provider HTTP response.
4. Independently confirmed the same path with a raw in-cell python script
   (CONNECT 200 -> TLS -> `GET /v1beta/models?key=FAKE` -> `HTTP/1.1 400 Bad
   Request` from Google's server with real Google response headers).

Conclusion: the firewall egress path to a real public provider is verified
working end-to-end through pi. A real API key is all that remains for a 200
model response — flagged for the developer.

Example update (committed in 65a705a): `examples/pi-bootstrap/Cellfile` had a
STALE comment ("network firewall enforcement is not yet implemented") — it
now is. Uncommented the Gemini `net.allow` as the default and documented the
verified `~/.pi`-seeding provider-precedence gotcha + the two workarounds
(drop the seeding block in `provision.sh`, or pass `--provider`/`--model`/
`--api-key` explicitly on `hotcell run -- pi`).

### Acceptance #3 — `cargo test --release` green (full release suite)
Includes the fixed `networked_agent_reaches_loopback_proxy`.

Pre-existing test bug fix (committed in e7fb3c3, tests/run_loopback_network.rs):
replaced the single `s.recv(128)` in `networked_agent_reaches_loopback_proxy`
with a read loop guarded by a 5s socket timeout that accumulates until
`b"hotcell-bridge"` is present (or EOF/timeout), because the echo server's
two `write_all` calls arrive as separate TCP segments. Confirmed
`cargo test --release --test run_loopback_network` passes 3/3 across 3
consecutive runs (was deterministically failing on trunk before the fix).

Full suite command + result:
`cargo test --release` -> exit 0. Per-binary results:
- unittests: 13 passed
- run_cell_override: 3 passed
- run_file_override: 4 passed
- run_firewall_integration: 1 passed
- run_global_flags: 4 passed
- run_loopback_network: 3 passed (incl. fixed test)
- run_minimal: 2 passed
- run_risk_profiles: 4 passed
- run_script_provisioner: 4 passed
Total: 38 passed, 0 failed.

### Lint (actually run, not claimed)
- `cargo fmt --check` -> exit 0 (clean).
- `cargo clippy --all-targets` -> exit 0, no warnings.

### Guard compliance
No foreground server left blocking: `hotcell run` invocations used `timeout`;
all test servers are in-process threads that exit with the test process.

### Flagged for the tech lead / developer
- Acceptance #2's "manually verified with an API key" still needs a REAL key
  in the developer's hands to observe a 200 model response. The path is
  proven; only the auth success is unverified.
- Consider whether `provision.sh` seeding `~/.pi` from the host should stay
  the default: it leaks the host's provider config into the cell and
  overrides the Cellfile's `env.PI_PROVIDER`. Two workarounds are documented
  in the example Cellfile.
