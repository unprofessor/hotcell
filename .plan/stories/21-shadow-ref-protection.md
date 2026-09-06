---
id: shadow-ref-protection
kind: story
parent: post-v1-workspace-sync
title: Opt-in per-ref push protection on the shadow
status: todo
assignee: null
created: 2026-09-06
updated: 2026-09-06
tags: [sync, git, hooks, policy]
depends_on: [shadow-repo-lifecycle]
---

## Goal

Let a developer protect chosen refs in a shadow from being clobbered, without
restricting the agent by default.

## Context

Deliberately narrow. An agent rebasing is fine — the structural isolation means
the blast radius is the shadow, not the developer's repo. The only thing this
protects is work the *developer* pushed into the shadow that the agent then
clobbers, and the developer still holds that locally. So it is opt-in, and its
value is avoiding confusion rather than preventing loss.

A blanket `receive.denyNonFastForwards` is repo-global and therefore the wrong
tool.

## Scope

- Per-ref policy by glob (git's own vocabulary — refspecs, `for-each-ref`),
  declared in the Cellfile, off by default.
- Enforcement via a generated `update` hook, which git invokes once per ref
  with `(refname, old_sha, new_sha)` and which can reject individually.
- **The hook convention.** This generates hotcell's first hook; a sanitizing
  `pre-receive` will be the second, and users may want their own. Settle a
  `hooks.d`-style dispatcher now — hotcell-managed hooks namespaced, user hooks
  alongside — or the later inspection work collides with this one. This is the
  architectural space boundary inspection needs; nothing else is reserved.

## Out of scope

- Boundary inspection itself. Only the room for it.
- Regex matching. Glob first; regex is additive later if something needs it,
  and starting with both means supporting both forever.

## Acceptance

- [ ] With no declaration, the shadow accepts any push including force.
- [ ] A declared protected glob rejects non-fast-forward pushes to matching
  refs only, leaving others untouched.
- [ ] The rejection message names the ref and why.
- [ ] A user-supplied hook and hotcell's generated hook coexist per the
  documented convention.
- [ ] Spec matches; `allium check` clean; `cargo fmt --check`, `cargo clippy`,
  `cargo test` clean.

## Notes

- 2026-09-06 filed from the settled design in [[post-v1-workspace-sync]].
