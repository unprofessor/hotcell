---
id: brokered-vcs-sync
kind: story
parent: post-v1-workspace-sync
title: Brokered VCS sync via a shadow remote
status: todo
assignee: null
created: 2026-09-05
updated: 2026-09-05
tags: [sync, vcs, bridge]
depends_on: [design-brokered-vcs-sync, rename-destroy-force-to-no-confirm, packages-seeds-semantics, guard-uds-socket-path-length]
---

## Status

Design settled; awaiting decomposition into stories. Do not claim this ticket
directly — it is being replaced by the story set the epic will carry.

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

## Settled design

Converged 2026-09-05/06. Recorded here so the decomposition can be cut from it.

### Workspace model

Cellfile declares logical repo *slots* (committed, portable); a machine-local
uncommitted file *binds* them to real paths. No declared slot means no sync
for that repo — binding is never implicit.

```kdl
// Cellfile — committed
repo "hotcell"
repo "planr" mount="/work/planr"    // optional; defaults to <workdir>/<name>

// .cell/bindings.kdl — gitignored, machine-local
bind "hotcell" path="/home/dave/projects/hotcell"
bind "planr"   path="/home/dave/projects/planr"
```

The precedent for host paths in a Cellfile is `provision.host_paths`
(`~/.nvm/...`, `~/.pi`), but those are conventional tool locations; a project
layout is not, and baking it in would break `ProvisionedEnvironment`'s
"reproducible from its Cellfile" claim (`hotcell.allium:381-388`).

**Binding the Cellfile's own repo is opt-in**, not implicit. It leaks the
Cellfile repo's contents into the cell; it is also how a cell improves its own
environment, behind a review gate, so it is worth having — just not by
default.

### Shadow lifecycle

**Birth** — bare-clone the real repo into the shadow; install a `shadow`
remote in the real repo; optionally (flag) clone from `origin` into the named
directory inside the cell, in case the agent should do that itself. Touch the
developer's repo only through safe `git -C` porcelain: it is a repo hotcell
does not own and whose state is unknown, so plug in a sink and a faucet, never
perform open-heart surgery. Bail on conflict or error and tell the user how to
make the repo accept a `shadow` remote. Roll back cleanly on failure at any
point — reuse the Death procedure rather than writing a separate unwind.

**Life** — ordinary git. The shadow is just a remote.

**Death** — Birth's steps in reverse, minus in-cell deletion (`destroy` owns
that): check for data loss, remove the `shadow` remote from the real repo,
remove the shadow bare repo, leave the in-cell repo alone. Reverse order
matters — never leave a remote pointing at a deleted path. A dangling in-cell
`origin` afterwards is honest about the remote's state, exactly as when a
forge goes down.

The data-loss check is what makes deletion a valid inverse of cloning: the
true inverse is "the shadow holds nothing the real repo lacks", not "delete
it". `destroy` calls this procedure rather than duplicating it, which also
gives the `--force` refusal one implementation site.

### Boundary inspection

Out of scope for v1; leave architectural space. Git hooks (local and broker)
are the intended gates, and a bare repo has a hooks directory by construction,
so nothing needs reserving. Alternatively a "sanitary" broker once a "simple"
one exists.

### Backend seam

Not yet. Write the git broker concretely and extract an interface once a
second implementation exists to reveal what is actually common. Keep
transport, repo lifecycle, and policy hooks in separate modules so the
extraction is mechanical. A third-party *plugin API* remains rejected — the
execution plan is the trust artifact (`src/plan.rs:16`), and this project
gates powerful extension points explicitly (cf. `approve-rego-policy-gate`).

### Spec first

The epic's first story defines the exchange concept in Allium before any code.

## Deferred

- **Bindings to arbitrary remote URLs.** Requires injecting auth the in-cell
  agent must not see — otherwise the CONNECT proxy already covers arbitrary
  remote access and this adds nothing. Auth injection is a gated-feature-sized
  change, not a resolver. The no-auth alternative is user-directed sync:
  bind the slot to a *local clone* that has the remote, and the developer
  pushes onward with their own credentials.
- **Shadow remote-tracking shim.** Two writers sharing one ref namespace in
  the shadow will collide on common branch names. That is ordinary git
  friction rather than breakage, but a default refspec convention at birth
  (e.g. host pushes land under their own prefix) would remove the coordination
  cost. Out-of-band from "standard git primitives", so it needs a shim layer.
  Deferred; revisit if sync becomes a PITA in practice.

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
