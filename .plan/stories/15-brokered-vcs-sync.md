---
id: brokered-vcs-sync
kind: story
parent: post-v1-workspace-sync
title: Brokered VCS sync via a shadow remote (STRAWMAN)
status: todo
assignee: null
created: 2026-09-05
updated: 2026-09-05
tags: [sync, vcs, bridge, strawman]
depends_on: [design-brokered-vcs-sync, rename-destroy-force-to-no-confirm, packages-seeds-semantics, guard-uds-socket-path-length]
---

## Status

**Strawman.** Design converged on the shape below in discussion; not yet
sized, not yet decomposed into tasks. Do not claim this — it needs the open
questions answered first.

## Shape

A bare repo per project, host-side, outside the cell rootfs, reached from
inside the cell over the existing UDS bridge. Asymmetric naming is the point:

```
in-cell working tree ──"origin"──▶ ┌────────────┐ ◀──"shadow"── host working tree
  (agent, plain git)               │ bare repo  │             (developer, plain git)
                                   └────────────┘
```

The agent sees an unremarkable `origin` and runs plain git. The developer
gains one extra remote and integrates with normal tooling. Neither side is
authoritative; it is two peers sharing a repo.

## Settled in discussion

- **No quarantine ref namespace needed.** `git fetch shadow` lands in
  `refs/remotes/shadow/*` by plain git semantics, so the developer's local
  branches are untouched by construction. The topology is the quarantine.
- **The agent needs zero hotcell awareness** — everything specific lives in
  `origin`'s URL and the helper behind it. Same property as `HTTPS_PROXY` for
  the firewall: an off-the-shelf agent works unmodified.
- **The bare repo is a sibling of `fs/`**, e.g. `.cell/<name>/git/<repo>.git`,
  never inside it — otherwise the agent reaches it through the filesystem and
  bypasses the broker entirely. Precedent: `src/state.rs:41-45` puts state
  markers outside `fs` for the same reason.
- **One socket, not one per repo**, with the repo name carried in the
  protocol. Keeps the socket path fixed and known-good (see
  [[guard-uds-socket-path-length]]), and gives one place for the
  allow/deny/scan decision with the repo as an audit field.
- **Prefer `ext::` or a `git-remote-hotcell` helper over a `git://` loopback
  hop** — removes port allocation, discovery and collision. Implementation
  shape is proven: bind the hotcell binary read-only under a second name and
  dispatch on `argv[0]`, as `hotcell fwd` already does
  (`src/isolation.rs:82-86`).
- **Site it at `/hotcell-bridge/`**, not under `/dev` — `/dev` is a fresh
  bwrap devtmpfs (`src/isolation.rs:211`), and `BRIDGE_DIR` already has the
  read-only-bind property established.
- **Destroy refuses on unfetched refs.** Confirmation and refusal are separate
  gates: `-y`/`--no-confirm` skips the prompt, `--force` overrides the
  refusal, refusal is checked *before* prompting. Full rationale in
  [[rename-destroy-force-to-no-confirm]].
- **Ref protection is opt-in and per-ref** (glob), via a generated `update`
  hook — not a blanket `receive.denyNonFastForwards`. An agent rebasing is
  fine given the structural isolation; the only thing this protects is work
  the developer pushed in that the agent then clobbers, which the developer
  still holds locally. Narrow by design.
- **This composes with the CONNECT proxy rather than replacing it**:
  `origin` → bridge, `upstream` → real remote. Two features.

## Open questions

1. **Workspace model.** There is no Cellfile construct for N repos — only a
   single `workdir` (`src/cellfile.rs:212-216`). A `repo` node (name, host
   path, in-cell mount) is needed, and it overlaps `seed`, which was dropped
   from the grammar and is still undecided (`src/cellfile.rs:239-250`). These
   should be one decision, not two — hence the dependency on
   [[packages-seeds-semantics]]. **This gates everything else.**
2. **Is ingress/egress inspection in scope?** Scanning and sanitizing at the
   boundary was the original motivation. If it is v1 of this feature the
   broker needs a policy surface and should share the decision/audit
   vocabulary being defined in [[firewall-warn-audit]]; if it is later, the
   broker is a dumb pipe and much smaller.
3. **Shadow lifecycle.** Is the bare repo seeded from the developer's real
   repo at cell start, and refreshed how? This is what buys the agent real
   history to log and rebase against — the thing a flat diff/export cannot
   give. Also: what happens to the shadow on `hotcell destroy` (see the
   refusal above).
4. **Shadow remote naming.** `shadow` for the default cell, `shadow-<cell>`
   otherwise? Plus collision handling when the host repo already has a remote
   by that name.
5. **Does a backend seam exist yet?** A `SyncBackend` enum/trait with the
   host-side reconciler as a second implementation, or one concrete broker
   until a second is real. A *plugin system* was argued against: the execution
   plan is the trust artifact (`src/plan.rs:16`), and this project's pattern
   for powerful extension points is an explicit approval gate (cf.
   `approve-rego-policy-gate`, `approve-tls-mitm-gate`), not an open door.
6. **Spec first?** `hotcell.allium` has seeding (`FileMapping`, one-shot,
   ingress-only) and persistence (`FilesPersistAfterSession`, 476-482) but no
   concept of *exchange*. This repo is spec-first; the gap probably wants an
   open question in the spec before code.

## Rejected

- **Live bind mount of the host project into the cell.** Simplest, but it
  contradicts `FilesystemIsolation` as written (`hotcell.allium:377-379`,
  "the agent cannot access any host filesystem path outside the sandbox") and
  `ProvisionedEnvironment`. Not impossible — it is a guarantee amendment, and
  must be argued as one rather than slipped in as a backend.
- **Host-side reconciler only** (`hotcell diff`/`export`). Kept on the books
  as the degenerate case — roughly this design with the transport removed —
  but it cannot carry history and does not handle N repos cleanly.

## Notes

- 2026-09-05 design is tracked by [[design-brokered-vcs-sync]], which gates
  this story. Open questions below are that task's agenda.
- 2026-09-05 stubbed as a strawman at the developer's request. Depends on
  [[rename-destroy-force-to-no-confirm]] so the `--force` reintroduction lands
  on the corrected flag surface.
