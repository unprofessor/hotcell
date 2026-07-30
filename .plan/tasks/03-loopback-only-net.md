---
id: loopback-only-net
kind: task
parent: network-firewall
title: Relax agent isolation to loopback-only plus proxy
status: in_progress
assignee: null
created: 2026-07-29
updated: 2026-07-29
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

- [ ] Agent profile allows loopback egress to the proxy's port only (e.g.
      bwrap without `--unshare-net` plus a loopback-only setup, or a net
      namespace with a loopback route to the proxy).
- [ ] Non-loopback egress remains blocked for the agent.
- [ ] An offline cell (empty policy) still has no network at all.
- [ ] Risk-profile isolation tests in `tests/run_risk_profiles.rs` still pass.

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
