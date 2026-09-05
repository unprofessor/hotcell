---
id: rename-destroy-force-to-no-confirm
aliases: [rename-destroy-force-to-no-confirm]
kind: task
parent: cli-surface-cleanup
title: Replace destroy --force with -y/--no-confirm
status: todo
assignee: null
created: 2026-09-05
updated: 2026-09-05
tags: [cli, ergonomics]
depends_on: []
---

## Goal

Make `destroy`'s confirmation bypass spell the same as `run`'s. Remove
`--force` outright — no alias, no deprecation period.

## Context

`run` skips its confirmation prompt with `-y`/`--no-confirm`
(`src/cli.rs:56-58`). `destroy` skips the same kind of prompt with `--force`
(`src/cli.rs:70-73`). Two gates, two spellings, for the same act.

`--force` is also a misnomer: it does not force anything past a check, it
declines to ask. Nothing else in the CLI uses "force" that way, and the word
invites the reading that it overrides the destroyable-state guard at
`src/cli.rs:319-327` — which it does not.

Note the short flag: `--force` is **long-only**, deliberately, because `-f` is
already the global `--file` (`src/cli.rs:32`, with a comment saying so). There
is no `-f`-for-force. `-y` is free on `destroy` and can be taken.

hotcell is pre-1.0 and `--force` has no documented users — `README.md` never
mentions it (only `-y` for `run`, line 137). Removing it cleanly beats
carrying a hidden alias.

## Approach

1. Replace `Destroy`'s `--force` with `-y`/`--no-confirm`, mirroring `Run`'s
   definition; rename the `force` field and the `destroy_cell` parameter to
   match, so the misnomer does not survive in the code.
2. Update callers: `tests/run_global_flags.rs:40-46`,
   `tests/destroy_confirmation.rs` (doc comment line 2 and the `--force` cases
   at 144-173).
3. Confirm nothing else references the flag. `.plan/tasks/38-*.md:183` mentions
   it but is a closed task record — leave it.

## Acceptance

- [ ] `hotcell destroy -y` and `hotcell destroy --no-confirm` bypass the
  prompt; behaviour is otherwise identical to today's `--force`.
- [ ] `hotcell destroy --force` is rejected as an unknown argument.
- [ ] `hotcell destroy --help` shows `-y`/`--no-confirm`.
- [ ] No `force` identifier remains on the destroy path in `src/cli.rs`.
- [ ] EOF and bare Enter on the prompt still mean "no" (`src/cli.rs:340-347`).
- [ ] Tests cover `-y` and the unchanged prompt default.
- [ ] `cargo fmt --check`, `cargo clippy`, `cargo test` clean.

## Notes

- 2026-09-05 created at the developer's request. "force" was identified as a
  misnomer; the flag declines to prompt rather than forcing past anything.
- 2026-09-05 developer decided against a deprecation alias: remove `--force`
  outright rather than hide-and-warn it.
