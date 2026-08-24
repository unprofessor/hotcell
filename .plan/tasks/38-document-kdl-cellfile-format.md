---
id: document-kdl-cellfile-format
aliases: [document-kdl-cellfile-format]
kind: task
parent: cellfile-kdl-grammar-and-secrets
title: Document the KDL Cellfile format
status: in_progress
assignee: null
created: 2026-08-23
updated: 2026-08-24
tags: [docs, cellfile]
depends_on: [replace-cellfile-parser-with-kdl, resolve-env-passthrough-and-secrets, redact-secrets-in-execution-plan]
---

## Goal

Update the README's Cellfile reference to the KDL grammar, document `env`
passthrough and `secret`, and raise the stated Rust prerequisite.

## Context

The README's "Cellfile format" section is the one reference no `--help`
surface covers, which is why it survived the trim — a user must author a
Cellfile before anything runs. It currently documents the retired
`key = value` format, including `package` and `seed`, which this story
removes.

Follow the `readme` skill: the ownership test, verify every example by running
it, and no status or roadmap content.

## Acceptance

- [~] The Cellfile reference documents the KDL grammar, including both `env`
  shapes and `secret`, with a worked example that has actually been run.
  PARTIAL: `workdir`, `provision`/`host-path`, literal `env NAME="value"`, and
  `net`/`allow`/`port` are documented and each example was run. The `env NAME`
  passthrough and `secret` shapes are NOT documented — see the blocker note.
- [x] `package` and `seed` are gone from the reference, not marked as
  unimplemented.
- [x] The endpoint form is `allow "host" port=443`; no `host:port` string.
- [x] Prerequisites state Rust 1.95+.
- [ ] The secrets behaviour is described where a user acts on it: values come
  from hotcell's environment, are redacted in the plan, are never written to
  cell state, and a missing one is an error. BLOCKED — none of this is true in
  the code yet (see blocker note); documenting it would be a false claim.
- [ ] `examples/pi-bootstrap/Cellfile` comments no longer tell the user to
  paste an API key into the file. BLOCKED — the only working env mechanism is
  the literal `env KEY="value"` the story is trying to retire; the `secret`
  replacement aborts at runtime.
- [x] Every command and Cellfile in the README has been executed as written
  (for everything documented — see the Validation section).

## Notes

- 2026-08-23 created.
- 2026-08-24 BLOCKER: this task's secrets criteria (#1 env passthrough +
  `secret`, #5 secrets behaviour, #6 example Cellfile) depend on
  [[resolve-env-passthrough-and-secrets]] and
  [[redact-secrets-in-execution-plan]], both still `status: todo`. `depends_on`
  lists only [[replace-cellfile-parser-with-kdl]], but that only makes the
  parser *accept* `env NAME` and `secret NAME`; the plan builder
  (`src/plan.rs`, `build_plan`) rejects both at runtime with "passthrough from
  the host environment is not yet implemented" / "secrets are not yet
  implemented". Verified live: `env PI_PROVIDER` and `secret ANTHROPIC_API_KEY`
  Cellfiles both abort under `hotcell run --dry-run`. Documenting them as
  working — resolved from hotcell's env, redacted in the plan, never persisted
  — would be an unverifiable false claim in a security tool, which the
  `readme` skill and the project's verify-claims/no-overclaiming doctrine
  forbid. The truthful current-state parts of the task were completed; the
  secrets documentation must wait until those two tasks land. Recommend the
  leader sequence them first, then re-dispatch the secrets portion of this
  task. Left `status: in_progress` for this reason.

## Validation

Built the worktree binary (`cargo build`; rustc 1.97.1, `rust-version = "1.95"`
in Cargo.toml) and ran every command and Cellfile that appears in the README
against it. bwrap 0.11.2 present.

- Quick-start Cellfile (`// Cellfile` comment + `workdir "/work"`) with
  `hotcell run -- echo hello from hotcell`: prompt gated, answered `y`, printed
  `hello from hotcell`, exit 0.
- Empty Cellfile: `hotcell run --dry-run` produced a plan with `--chdir /work`
  (default workdir applied), confirming "a completely empty Cellfile is valid".
- Cellfile-format worked example (`workdir`, `provision "shell"` with two
  `host-path` children, literal `env PATH=...`, `net { allow "..." port=443;
  allow "example.com" }`), run as written with a `provision.sh` present:
  `hotcell run --dry-run -- pi` emitted both the provisioning and agent bwrap
  steps, `--ro-bind` for `~/.nvm/versions/node` and `~/.gitconfig`, and
  `--setenv PATH /opt/node/bin:/usr/bin:/bin`. Both host paths exist locally.
- Commands block: `run --dry-run -- pi`, `run --dry-run --name scratch -- sh`
  (plan targets `.cell/scratch/fs`), `status` (`no cells in ...`), `run -y`
  then `destroy` (aborts on empty input; `echo y | destroy` printed
  `destroyed cell "default"`), `--help`, `run --help` — all exit 0 / behave as
  documented.
- Stale-token scan of README: no `package`, `seed`, `host:port`, `net.allow`,
  `env.`, `provision.type`, `1.81`, or `key = value` remain (only the unrelated
  "package manager" phrase in the bubblewrap install line).

NOT validated because unimplemented (see blocker note): `env NAME` passthrough
and `secret NAME`. Verified they abort rather than work — a `Cellfile` with
`env PI_PROVIDER` fails with "passthrough from the host environment is not yet
implemented" and one with `secret ANTHROPIC_API_KEY` fails with "secrets are
not yet implemented", both under `hotcell run --dry-run`. Neither is documented
in the README.
