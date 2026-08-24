---
id: wire-or-remove-provisioner-override
aliases: [wire-or-remove-provisioner-override]
kind: task
parent: reconcile-spec-code-divergences
title: Wire or remove provisioner_override from the CLI surface
status: todo
assignee: null
created: 2026-08-24
updated: 2026-08-24
tags: [spec, allium, cli, provisioning]
depends_on: []
---

## Goal

Make the spec's `provisioner_override` match the CLI: either add the flag that
supplies it, or drop it from the spec's CLI surface.

## Context

The spec's `DeveloperCLI.RunCell` takes an optional `provisioner_override`
(`hotcell.allium:429-431`, "when given, replaces the Cellfile's provisioner").
There is no corresponding flag or argument in `src/cli.rs` — the CLI offers no
way to supply it, so the parameter is unreachable.

## Approach

Decide and align:

1. **Remove from spec.** If overriding the provisioner from the command line is
   not a v1 capability, delete `provisioner_override` from `RunCell` and its
   prose. Cheapest; keeps the spec honest.
2. **Add the flag.** If the override is wanted (e.g. `--provisioner none` to
   skip provisioning for a run), add it to `src/cli.rs`, thread it through the
   plan build, and keep the spec — but that is a real feature and should be
   sized as such, possibly its own task/story rather than a reconciliation.

Prefer option 1 unless the tech lead wants the feature; if option 2, record the
decision and split the feature out rather than growing this task.

## Acceptance

- [ ] The decision (remove vs. implement) is recorded in Notes with rationale.
- [ ] If removed: `provisioner_override` no longer appears in the spec's CLI
  surface; `allium check` reports 0 errors and no fewer diagnostics resolved.
- [ ] If implemented: split into its own task/story; this task records that and
  does not itself grow into a feature.
- [ ] `cargo test` still green (and `fmt`/`clippy` clean if code changed).

## Notes

- 2026-08-24 created from the `weed` pre-existing findings in
  [[realign-allium-spec-with-env-sources]]. Likely resolution: remove it from
  the spec — no product need for a CLI provisioner override has been stated for
  v1.
