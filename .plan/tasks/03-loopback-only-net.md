---
id: loopback-only-net
kind: task
parent: network-firewall
title: Relax agent isolation to loopback-only plus proxy
status: review
assignee: null
created: 2026-07-29
updated: 2026-07-30
tags: []
depends_on: [wire-firewall-into-cli]
---

## Goal

Relax agent isolation to loopback-only plus proxy

## Context

Parent: `network-firewall`. File: `src/isolation.rs` (`build_agent_command`,
currently `--unshare-net`). The proxy listens on loopback; the agent must reach
it but nothing else. `--unshare-net` blocks even loopback to the host proxy,
so a network policy cannot be enforced while it's set.

## Acceptance

- [x] Agent profile allows loopback egress to the proxy's port only (e.g.
      bwrap without `--unshare-net` plus a loopback-only setup, or a net
      namespace with a loopback route to the proxy).
- [x] Non-loopback egress remains blocked for the agent.
- [x] An offline cell (empty policy) still has no network at all.
- [x] Risk-profile isolation tests in `tests/run_risk_profiles.rs` still pass.

## Notes

- 2026-07-29 created. Depends on `wire-firewall-into-cli`; coordinate with it
  since both touch the agent launch path.
- 2026-07-29 tech-lead: design decision settled (approach B from the task
  brief). Pre-seeded here so the worker implements, not re-derives:

  **Approach B — loopback-only namespace + Unix-socket bridge to the host proxy.**

  Keep `--unshare-net`. The namespace then has ONLY a private loopback
  interface, so non-loopback egress is kernel-enforced "Network is
  unreachable" — no route/interface to tamper with, no `CAP_NET_ADMIN`
  needed. This satisfies acceptance #2 airtight and is more defensible than
  passt/routing approaches (A/C), which leave an outbound route that must be
  guarded by rules.

  Cross the namespace boundary with a Unix-domain socket bridge so the
  agent can reach the host-side firewall proxy WITHOUT any network route
  being exposed:
    1. The host firewall proxy (already started by `wire-firewall-into-cli`
       on the host's 127.0.0.1:<port>) is the real egress point.
    2. A small in-namespace forwarder process listens on the namespace's
       own 127.0.0.1:<proxy-port> and relays bytes to the host proxy via a
       Unix-domain socket that lives in a directory bind-mounted into both
       the host and the namespace (bwrap `--bind` of a shared cell-local
       dir, e.g. under the cell fs).
    3. The agent's `HTTP_PROXY`/`HTTPS_PROXY` point at the in-namespace
       forwarder's 127.0.0.1:<port>, so from the agent's view the proxy is
       on its own loopback — reachable, and nothing else is.

  The forwarder must be a tiny, self-contained binary or a stdlib-based
  process started by hotcell inside the namespace alongside the agent.
  Prefer reusing hotcell's own tokio/hyper deps for the forwarder, or a
  minimal `std::net::TcpListener` + `std::os::unix::net::UnixStream` copier
  if that's simpler. Do NOT rely on external tools (nc/socat) inside the
  sandbox — keep it in-process/hotcell-owned so the cell is hermetic.

  Empty-policy (offline) cells: do NOT start a forwarder and do NOT bind the
  shared socket dir; keep `--unshare-net` as today. Offline stays airtight.

  Residual risk to document: the Unix-socket bridge trusts the host-side
  socket path; document that the shared dir is cell-scoped (under the cell
  fs) so no other cell/host process can squat it. If you find a cleaner
  boundary (e.g. an abstract socket namespace, or an fd passed via
  bwrap/scm_rights), note it as a future improvement — do not block v1 on it.

  Open question for the worker to resolve by experiment: how to start the
  in-namespace forwarder. Two candidates:
    a. hotcell spawns a second `bwrap`-launched process (a `hotcell fwd`
       subcommand) inside the SAME net namespace as the agent, then execs
       the agent in the same namespace; or
    b. hotcell launches a single bwrap that runs a small supervisor which
       starts the forwarder then execs the agent.
  Pick whichever is cleanest with bwrap 0.11.2 unprivileged. Investigate,
  decide, document in ## Validation.

  Implementation guard (from the tech lead): when running network
  experiments, NEVER leave a server in the foreground. Always:
    - wrap experiments in `timeout 10 ...`
    - background servers (`... &`) and capture their PID, then `kill` them
    - prefer one-shot round-trip checks (connect, send, recv, exit) over
      long-lived servers.
  A blocking foreground command will stall the turn.

- 2026-07-30 worker: implemented Approach B. Threat model and residual risk
  writeup below; see `## Validation` for the experiment + test results.

  ## Threat model (Approach B, as implemented)

  **Architecture (data path for a networked cell):**

  ```
  agent (in bwrap --unshare-net namespace)
    |  TCP CONNECT to 127.0.0.1:<fwd-port>  (namespace's own loopback)
    v
  in-namespace forwarder = `hotcell fwd` supervisor
  (hotcell binary re-invoked, bind-mounted RO at /hotcell-fwd)
    |  raw byte relay TCP -> Unix-domain socket at /hotcell-bridge/proxy.sock
    v
  Unix-domain socket (filesystem; crosses netns boundary, NOT network)
    |  in a cell-scoped dir under the cell rootfs, RO-bound into the sandbox
    v
  host-side UDS bridge = `FirewallHandle::start_uds_bridge`
  (runs on the firewall's existing tokio runtime, on the HOST)
    |  raw byte relay Unix -> TCP 127.0.0.1:<proxy-port>  (host loopback)
    v
  firewall proxy = `firewall::start` HTTP CONNECT allowlist
    |  CONNECT allowlisted -> tunnel; else 403
    v
  external network (host)
  ```

  Both relay hops (namespace forwarder, host UDS bridge) are **dumb byte
  pipes**; all policy enforcement stays in the firewall proxy's
  `handle_connect`. The agent never sees a network route.

  **Airtight:**
  - **Non-loopback egress blocked (acceptance #2):** the namespace created by
    `bwrap --unshare-net` has *only* a private loopback interface — there is
    no route and no other interface for any non-loopback destination. A
    `connect()` to a non-loopback address returns `ENETUNREACH` (errno 101)
    from the kernel, before any userspace policy is consulted. There is no
    route to tamper with and no `CAP_NET_ADMIN` inside the sandbox. This is
    enforced identically for offline and networked cells (`--unshare-net` is
    kept in both paths). Verified:
    `tests/run_loopback_network.rs::networked_agent_cannot_reach_non_loopback`.
  - **Offline cell has no network (acceptance #3):** empty policy starts no
    firewall, no bridge, no forwarder, and keeps `--unshare-net`. No
    `HTTP_PROXY`/`HTTPS_PROXY` env is set. Verified:
    `offline_cell_has_no_network`.
  - **Loopback egress to the proxy only (acceptance #1):** the agent can only
    open TCP to its own loopback, where the *single* listener is the
    hotcell-owned forwarder. The forwarder relays only to the bridge UDS, which
    relays only to the firewall, which allowlists `CONNECT` destinations. An
    agent CONNECT to an endpoint not in the policy is `403` at the firewall.
    Verified: `networked_agent_reaches_loopback_proxy` (allowed -> 200 +
    tunnel) and the manual e2e (unallowed -> 403).
  - **Bridge dir tamper-resistance:** `/hotcell-bridge` is bind-mounted
    **read-only** into the sandbox, so the agent cannot write to or replace
    the bridge socket. The forwarder (hotcell-owned code, not the agent) is
    the only thing that connects to it.
  - **Supervisor binary tamper-resistance:** the hotcell binary is bind-mounted
    **read-only** at `/hotcell-fwd` and invoked with a fixed `fwd` subcommand;
    the agent cannot modify it or influence its arguments.

  **Residual risk (documented, accepted for v1):**
  - **DoS, not escalation, via the bridge dir.** The bridge dir lives *under
    the cell rootfs* (which is `--bind` RW as `/`), so in principle the agent
    could remove/recreate `/hotcell-bridge` before the RO bind applies. bwrap
    applies operations in order and the explicit `--ro-bind /hotcell-bridge`
    is emitted *after* the rootfs bind, so the agent's view is read-only at
    runtime; but the host-side socket file is created on the host path and the
    RO bind shadows it. The worst the agent can achieve is breaking its own
    proxy access (self-DoS) — it cannot escalate egress, because (a) it has no
    network route, and (b) even if it ran a fake UDS listener, the forwarder
    would connect to it and the agent would only intercept its *own* traffic,
    gaining no additional egress. Not an escalation.
  - **Bridge dir is visible to the agent at `/hotcell-bridge`.** This leaks
    the existence of the bridge (a single empty dir + socket filename) but no
    host state. The dir is removed after the agent exits. Future: stage it
    under the existing `/hotcell` staging prefix and clean it like host-path
    stubs, so the agent sees no bridge artifact at all.
  - **Host loopback reachability of the proxy is host-trusted.** The firewall
    proxy binds the *host's* `127.0.0.1`. Any host process can connect to it.
    This is unchanged from the pre-bridge design and is a host-side trust
    assumption (the host is trusted; the sandbox is not). The bridge does not
    widen this.
  - **Forwarder port is OS-assigned in the namespace** (binds `127.0.0.1:0`),
    so no fixed-port collision with an agent service. The forwarder discovers
    its own port and sets `HTTP_PROXY` for the agent child — the agent never
    needs to know a fixed port.

  **Future improvement (not blocking v1):** pass the bridge socket as an fd
  via `SCM_RIGHTS` / bwrap `--fd` instead of a filesystem path, eliminating
  the bridge-dir-on-rootfs artifact entirely. Or use the Linux abstract
  socket namespace (no filesystem path) — but abstract sockets are
  host-global and would leak across cells, so the cell-scoped filesystem path
  is the more defensible v1.

  **Open question resolution:** chose **option (b)** — a single `bwrap`
  invocation that runs the `hotcell fwd` supervisor (the hotcell binary
  re-invoked and bind-mounted RO). The supervisor starts the in-namespace
  loopback TCP->UDS forwarder, discovers its port, sets `HTTP_PROXY` for the
  agent, and spawns the agent in the same `--unshare-net` namespace. This is
  cleanest with bwrap 0.11.2 unprivileged: option (a) (two bwrap processes
  sharing one freshly-created netns) would require creating the namespace
  out-of-band and having both join it, which needs `unshare`/`nsenter` and is
  more fragile unprivileged. A single bwrap + supervisor keeps the namespace
  lifecycle in bwrap's hands. Verified executable: the bind-mounted hotcell
  binary runs `--help` inside `bwrap --unshare-net`.

## Validation

- Experiment 1 — UDS crosses the `--unshare-net` boundary: a Python UDS echo
  server on the host, with its dir `--ro-bind`-mounted into a
  `bwrap --unshare-net` sandbox; the in-sandbox client connected, sent
  `hello`, received `echo:hello`. `exit=0`. Confirms the bridge transport.
- Experiment 2 — `--unshare-net` blocks non-loopback egress: in-sandbox
  `socket.create_connection(("1.1.1.1", 80))` raised `OSError [Errno 101]
  Network is unreachable`. Confirms acceptance #2's kernel enforcement.
- Experiment 3 — bind-mounted hotcell binary executes in-sandbox:
  `bwrap --unshare-net --ro-bind <hotcell> /hotcell-fwd ... /hotcell-fwd --help`
  printed the help banner. Confirms the supervisor approach (option b).
- Manual end-to-end (`/tmp/e2e-bridge`, host echo server on an OS-assigned
  port, policy `net.allow = 127.0.0.1:<port>`):
  - allowed CONNECT via `$HTTP_PROXY` -> `HTTP/1.1 200 OK` + tunnel echo
    `echo:ping`;
  - direct non-loopback TCP -> `blocked errno=101`;
  - CONNECT to an unallowed port -> `HTTP/1.1 403 Forbidden`.
  `ALL_OK`, `exit=0`.
- Manual offline (`/tmp/e2e-offline`, empty policy):
  - `HTTP_PROXY unset: True`, `HTTPS_PROXY` unset;
  - non-loopback -> `blocked errno=101`.
  `ALL_OK`, `exit=0`.
- Automated tests: `cargo test` (full suite) — 30 tests pass, 0 fail:
  - `tests/run_loopback_network.rs` (new, 3 tests): offline has no network;
    networked agent reaches allowed loopback endpoint via the proxy (200 +
    tunnel echo); networked agent cannot reach non-loopback (ENETUNREACH).
  - `tests/run_risk_profiles.rs` (4 tests, unchanged): still pass
    (acceptance #4 — no regression to the offline path or risk-profile
    isolation).
  - `tests/run_script_provisioner.rs` (4), `tests/run_cell.rs` (2), firewall
    unit tests (9), other unit tests: all pass.
- `cargo clippy --all-targets`: clean (0 warnings, 0 errors).
- Acceptance criteria:
  1. Agent profile allows loopback egress to the proxy's port only — PASS
     (forwarder on namespace loopback; firewall allowlists CONNECT).
  2. Non-loopback egress remains blocked — PASS (`--unshare-net` =>
     `ENETUNREACH`, kernel-enforced, tested).
  3. Offline cell (empty policy) still has no network — PASS (no firewall /
     forwarder / proxy env; `--unshare-net`; tested).
  4. Risk-profile isolation tests still pass — PASS (4/4, no regression).

  No fully-airtight piece was found unachievable; the only residuals are
  documented self-DoS / cosmetic-visibility items, none of which are egress
  escalations. No gap to flag to the tech lead as blocking.
