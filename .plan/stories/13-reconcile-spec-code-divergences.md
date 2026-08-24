---
id: reconcile-spec-code-divergences
kind: story
parent: post-v1-cellfile-and-provisioning
title: Reconcile hotcell.allium with code (provisioning/session drift)
status: todo
assignee: null
created: 2026-08-24
updated: 2026-08-24
tags: [spec, allium, provisioning]
---

## Goal

Close four pre-existing divergences between `hotcell.allium` and the code,
where the spec asserts behaviour the code does not implement (or names it
differently). For each, decide which side is authoritative and make them agree
— by fixing the code, correcting the spec, or explicitly marking the spec
construct descriptive.

## Context

The `weed` audit run under
[[realign-allium-spec-with-env-sources]] confirmed five spec↔code divergences.
One (`packages`/`files`) is already owned by [[packages-seeds-semantics]]. The
other four had no owning ticket and are collected here:

1. **Provisioner kind name.** Spec's `ProvisioningProcess` lists kinds
   `"none"` and `"script"`; the code implements `"none"` (default) and
   `"shell"`, and rejects `"script"` as "unknown provisioner kind"
   (`src/provisioning.rs:88-99,131-134`; `src/state.rs:30`;
   `src/cellfile.rs:114,320`). — task [[reconcile-provisioner-kind-name]].
2. **`Session` unenforced.** Spec models a `Session` entity, a
   `SessionCompletes` rule, `active_sessions`, and guarantees, and
   `DestroyCell` requires `active_sessions.count = 0`; no session is recorded
   at runtime (`allium check` reports `Session.*` fields unused; no `Session`
   construct in `src/`). — task [[reconcile-session-modeling]].
3. **`provisioner_override` has no CLI flag.** In the spec's
   `DeveloperCLI.RunCell(...provisioner_override?)` (`hotcell.allium:429`);
   no corresponding flag or field in `src/cli.rs`. — task
   [[wire-or-remove-provisioner-override]].
4. **`provisioning_error` discarded.** Spec retains a `provisioning_error`
   string on the cell (`hotcell.allium:159,330`); the code writes only a
   boolean `failed` marker and drops the message (`src/state.rs:33,91,139`;
   `src/provisioning.rs:161`). — task [[persist-or-drop-provisioning-error]].

Each divergence pre-dates the KDL/secrets story and was left for the leader to
ticket rather than fixed inside a worker's spec-alignment task.

## Scope

- The four reconciliations above, each as its own task.
- For each: a decision (code is right / spec is right / spec is descriptive)
  followed by the aligning change, and `allium check` left at 0 errors.

## Out of scope

- `packages`/`files` — owned by [[packages-seeds-semantics]].
- The env-source spec work — already done in
  [[realign-allium-spec-with-env-sources]].
- Any new capability; this story only makes spec and code tell the same story.

## Notes

- 2026-08-24 created by the leader from the pre-existing divergences confirmed
  by `weed` in [[realign-allium-spec-with-env-sources]]. The `Session` item is
  session-lifecycle spec hygiene rather than strictly Cellfile/provisioning; it
  is homed here for lack of a session epic and can rehome if one appears.
