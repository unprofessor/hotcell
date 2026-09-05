---
id: rename-destroy-force-to-no-confirm
aliases: [rename-destroy-force-to-no-confirm]
kind: task
parent: cli-surface-cleanup
title: Rename destroy --force to -y/--no-confirm and deprecate --force
status: todo
assignee: null
created: 2026-09-05
updated: 2026-09-05
tags: [cli, ergonomics, deprecation]
depends_on: []
---

## Goal

Make `destroy`'s confirmation bypass spell the same as `run`'s, and retire
`--force`.

## Context

`run` skips its confirmation prompt with `-y`/`--no-confirm`
(`src/cli.rs:56-58`). `destroy` skips the same kind of prompt with `--force`
(`src/cli.rs:70-73`). Two gates, two spellings, for the same act.

`--force` is also a misnomer: it does not force anything past a check, it
declines to ask. Nothing else in the CLI uses "force" to mean that, and the
word invites the reading that it overrides the destroyable-state guard at
`src/cli.rs:319-327` — which it does not.

Note the short flag: `--force` is **long-only**, deliberately, because `-f` is
already the global `--file` (`src/cli.rs:32`, with a comment saying so). So
there is no `-f`-for-force to deprecate — only `--force`. `-y` is free on
`destroy` and can be taken.

## Approach

1. Add `-y`/`--no-confirm` to `Destroy`, mirroring `Run`'s definition.
2. Keep `--force` as a hidden deprecated alias (`#[arg(hide = true)]`) that
   sets the same field and emits a one-line deprecation notice to stderr, so
   existing scripts keep working and their owners find out.
3. Update callers: `tests/run_global_flags.rs:40-46`,
   `tests/destroy_confirmation.rs` (doc comment line 2 and the `--force` cases
   at 144-173). Add a case asserting `-y` bypasses the prompt and one
   asserting `--force` still works and warns.
4. Decide and record when the alias is removed (a release, or "next breaking
   change"), so the deprecation has an end.

`README.md` does not mention `--force` (only `-y` for `run`, line 137), so no
README change is required — confirm before assuming.

## Acceptance

- [ ] `hotcell destroy -y` and `hotcell destroy --no-confirm` bypass the
  prompt; behaviour is otherwise identical to today's `--force`.
- [ ] `hotcell destroy --force` still bypasses the prompt, is hidden from
  `--help`, and prints a deprecation notice to stderr (stdout stays parseable).
- [ ] `hotcell destroy --help` shows `-y`/`--no-confirm` and not `--force`.
- [ ] EOF and bare Enter on the prompt still mean "no" (`src/cli.rs:340-347`).
- [ ] Tests cover `-y`, the deprecated `--force`, and the unchanged prompt
  default.
- [ ] The removal point for the alias is recorded in Notes.
- [ ] `cargo fmt --check`, `cargo clippy`, `cargo test` clean.

## Notes

- 2026-09-05 created at the developer's request. "force" was identified as a
  misnomer; the flag declines to prompt rather than forcing past anything.
