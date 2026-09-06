---
id: shadow-repo-lifecycle
kind: story
parent: workspace-sync
title: Shadow repo Birth and Death
status: todo
assignee: null
created: 2026-09-06
updated: 2026-09-06
tags: [sync, lifecycle, git, destroy]
depends_on: [workspace-repo-model, rename-destroy-force-to-no-confirm]
---

## Goal

Create and tear down a shadow repo as a strict inverse pair, driven by a
reconciler rather than by change events, and make `destroy` delegate to it
rather than duplicate it.

## Reconciliation, not change detection

Shadows are brought in line by comparing **present state to present
declaration**, never against a history:

| declaration | `.cell/<n>/git/` | action |
|---|---|---|
| slot | no shadow | Birth |
| no slot | shadow | Death (orphan) |
| slot | shadow | nothing |

A slot rename is therefore one Death plus one Birth, which is the correct
semantics anyway — a renamed slot *is* a different repo as far as the cell is
concerned. Consequences worth keeping:

- **No snapshot history, and so no ABA problem.** Comparing against present
  state cannot be fooled by a change that reverts itself between checks.
- **Self-healing.** Survives hand edits to either side, an interrupted run, a
  restored `.cell` backup, or a shadow deleted by hand.
- **Intent is irrelevant because the data-loss check is unconditional.** An
  orphan holding unfetched refs refuses regardless of whether it arose from a
  rename, a deletion, or a typo. Rename detection was only ever a proxy for
  that property.

Do **not** route this through `provisioned_as.json`. That record's lifecycle is
provisioning — `begin_provisioning` clears it and `mark_provisioned` rewrites
it (`src/state.rs:107-131`) — so shadows would die whenever a rootfs is
rebuilt. Putting repo slots in the declaration would also put them in cell
identity, making a slot rename force a full re-provision: a second instance of
the bug [[env-changes-should-not-reprovision]] exists to fix.

## Scope

**Birth** — bare-clone the bound repo into the shadow; record the host path it
was born from inside the shadow; install a `shadow` remote in the real repo;
optionally (flag) clone from `origin` into the cell, in case the agent should
do it instead.

The recorded host path is provenance: an orphan's refusal can name the repo it
came from rather than only a dead slot, it makes a stale shadow debuggable, and
it is the input a rename-migration prompt would need if one is ever wanted
(one orphan plus one new slot resolving to the same host path is a rename with
high confidence). Not required for correctness — reconciliation works without
it.

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
- [ ] Reconciliation converges from any starting state: missing shadow, orphan
  shadow, both, neither. Running it twice changes nothing the second time.
- [ ] A slot rename reconciles as Death + Birth, and refuses if the orphan
  holds unfetched refs.
- [ ] A shadow records its origin host path, and an orphan's refusal names it.
- [ ] No shadow state is read from or written to `provisioned_as.json`;
  re-provisioning a cell does not disturb its shadows.
- [ ] `cargo fmt --check`, `cargo clippy`, `cargo test` clean.

## Notes

- 2026-09-06 filed from the settled design in [[workspace-sync]].
