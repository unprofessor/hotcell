---
id: reconcile-session-modeling
aliases: [reconcile-session-modeling]
kind: task
parent: reconcile-spec-code-divergences
title: Reconcile the Session entity with runtime behaviour
status: todo
assignee: null
created: 2026-08-24
updated: 2026-08-24
tags: [spec, allium, session]
depends_on: []
---

## Goal

Make `hotcell.allium`'s `Session` model match what the code actually tracks —
either by recording sessions at runtime, or by marking the `Session` construct
descriptive so the spec stops asserting an enforcement the code does not do.

## Context

The spec models a `Session` entity (`hotcell.allium:191`), a `SessionCompletes`
rule (line 337), a `Cell.sessions` / `active_sessions` relationship, and
guarantees (`SessionLogsToFile`, `PostSessionReport`, `FilesPersistAfterSession`).
Critically, `DestroyCell` is specified with a precondition that
`active_sessions.count = 0`. The code records **no session** at runtime — there
is no `Session` construct in `src/`, and `allium check` reports the `Session.*`
fields as unused. So the `active_sessions.count = 0` precondition, as written,
is a guarantee the code cannot honour because it never counts sessions.

This is session-lifecycle spec hygiene, broader than Cellfile/provisioning; it
is homed under [[reconcile-spec-code-divergences]] for lack of a session epic.

## Approach

Decide which is true and align to it:

1. **Spec is aspirational / descriptive.** If concurrent-session tracking is
   not a v1 guarantee, rework the `Session` entity and the `DestroyCell`
   precondition so the spec describes single-invocation behaviour without
   asserting an `active_sessions` count the code never maintains — or annotate
   `Session` as an external/descriptive concept per Allium conventions so
   `allium check` stops reporting its fields unused.
2. **Code should track sessions.** If the `active_sessions.count = 0` guard on
   `DestroyCell` is genuinely wanted (e.g. refuse to destroy a cell with a live
   session), that is a code change — but likely larger than a spec-alignment
   task and may deserve its own story rather than being forced in here.

Prefer option 1 for this task unless the tech lead wants the enforcement; if
option 2, record that and split a code story rather than expanding scope.

## Acceptance

- [ ] The decision (descriptive spec vs. implement tracking) is recorded in
  Notes with rationale.
- [ ] If descriptive: `Session`, `active_sessions`, the `DestroyCell`
  precondition, and the related guarantees no longer assert runtime behaviour
  the code omits; `allium check` no longer reports `Session.*` as an unenforced
  guarantee gap (unused-field diagnostics may remain if intentionally external,
  and are explained in Notes).
- [ ] If implementing tracking: scope is split into its own story and this task
  records that decision instead of making the code change.
- [ ] `allium check hotcell.allium` reports 0 errors; `cargo test` still green.

## Notes

- 2026-08-24 created from the `weed` pre-existing findings in
  [[realign-allium-spec-with-env-sources]]. Likely resolution: make the spec
  honest about single-invocation behaviour rather than build session tracking
  for v1 — but the `DestroyCell` guard is a real question for the tech lead.
