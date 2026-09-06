---
id: shadow-repo-lifecycle
kind: story
parent: post-v1-workspace-sync
title: Shadow repo Birth and Death
status: todo
assignee: null
created: 2026-09-06
updated: 2026-09-06
tags: [sync, lifecycle, git, destroy]
depends_on: [workspace-repo-model, rename-destroy-force-to-no-confirm]
---

## Goal

Create and tear down a shadow repo as a strict inverse pair, and make
`destroy` delegate to it rather than duplicate it.

## Scope

**Birth** — bare-clone the bound repo into the shadow; install a `shadow`
remote in the real repo; optionally (flag) clone from `origin` into the cell,
in case the agent should do it instead.

**Death** — Birth in reverse, minus in-cell deletion (`destroy` owns that):
data-loss check, remove the `shadow` remote from the real repo, remove the
shadow bare repo, leave the in-cell repo alone.

Constraints that are the point of this story, not incidental:

- **Safe porcelain only** on the developer's repo. hotcell does not own it and
  cannot know its state: `git -C` porcelain to plug in a sink and a faucet, no
  open-heart surgery.
- **Bail and advise** on conflict or error — tell the user how to make the repo
  accept a `shadow` remote rather than forcing it.
- **Death is also Birth's rollback.** A partial Birth is unwound by running
  Death, so Death must tolerate missing pieces and there is no separate unwind
  path.
- **Reverse order matters** — never leave a `shadow` remote pointing at a
  deleted path.
- **The data-loss check is what makes Death a valid inverse of Birth.** The
  true inverse of cloning is "the shadow holds nothing the real repo lacks",
  not "delete it".
- A dangling in-cell `origin` after Death is correct and left alone; it is
  honest about the remote's state, as when a forge goes down.
- If Birth's in-cell clone flag is off, the agent still needs to discover the
  URL — decide the mechanism (env var per slot, or a manifest in the workdir).

**`destroy` integration** — calls this procedure instead of duplicating it,
which also gives the refusal exactly one implementation site. Confirmation and
refusal stay separate gates: `-y`/`--no-confirm` skips the prompt, `--force`
overrides the refusal, refusal is checked *before* prompting. This is where
`--force` is reintroduced with its corrected meaning; see
[[rename-destroy-force-to-no-confirm]].

## Acceptance

- [ ] Birth is idempotent and leaves the developer's repo working; a conflict
  aborts with actionable guidance and no partial state.
- [ ] Death removes exactly what Birth added, in reverse order, and tolerates
  missing pieces.
- [ ] A failed Birth cleaned up by Death leaves no trace in the real repo.
- [ ] `destroy` refuses when the shadow holds unfetched refs, before prompting;
  `--force` overrides; `-y` alone does not.
- [ ] `--force` is a no-op when there is no shadow or nothing unfetched.
- [ ] Refusal logic exists in one place, called by both paths.
- [ ] `cargo fmt --check`, `cargo clippy`, `cargo test` clean.

## Notes

- 2026-09-06 filed from the settled design in [[post-v1-workspace-sync]].
