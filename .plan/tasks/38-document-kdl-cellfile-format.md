---
id: document-kdl-cellfile-format
aliases: [document-kdl-cellfile-format]
kind: task
parent: cellfile-kdl-grammar-and-secrets
title: Document the KDL Cellfile format
status: review
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

- [x] The Cellfile reference documents the KDL grammar, including both `env`
  shapes (literal `env NAME="value"` and passthrough `env NAME`) and
  `secret NAME`, with a worked example that has actually been run.
- [x] `package` and `seed` are gone from the reference, not marked as
  unimplemented.
- [x] The endpoint form is `allow "host" port=443`; no `host:port` string.
- [x] Prerequisites state Rust 1.95+.
- [x] The secrets behaviour is described where a user acts on it (new
  "Environment variables and secrets" subsection): values come from hotcell's
  environment, a secret is shown as `<redacted>` in the plan with a footer that
  the block won't run verbatim, a missing declared var is a hard error before
  any state, and resolved values are never written to cell state.
- [x] `examples/pi-bootstrap/Cellfile` uses `secret GOOGLE_API_KEY` (the
  default provider's key, with `secret ANTHROPIC_API_KEY` / `secret
  OPENAI_API_KEY` as commented alternatives) instead of a literal
  `env KEY="value"`, and the "paste your key here" leak warning is gone.
- [x] Every command and Cellfile in the README (and the example) has been
  executed as written — see the Validation section.

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
- 2026-08-24 BLOCKER CLEARED: the leader rebased this branch onto main with
  [[resolve-env-passthrough-and-secrets]] and
  [[redact-secrets-in-execution-plan]] merged. Rebuilt and verified live that
  both `env` shapes and `secret` now work; added the passthrough/secret docs, a
  new "Environment variables and secrets" README subsection, and the
  `examples/pi-bootstrap/Cellfile` `secret` fix. All acceptance boxes now met;
  status set to `review`. See the extended Validation below.

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

Secrets validation (2026-08-24, after the rebase; rebuilt binary). Every one
run live, secret value `sk-supersecret-123` / `sk-secret-xyz` / fake keys:

- **Passthrough resolves**: `env PI_PROVIDER` with `PI_PROVIDER=google` in
  hotcell's env → plan shows `--setenv PI_PROVIDER google`.
- **Secret redacts**: `secret ANTHROPIC_API_KEY` with the var set → plan shows
  `--setenv ANTHROPIC_API_KEY <redacted>` plus the footer "a declared secret is
  shown as <redacted>: this block will NOT run verbatim as shown …". The real
  value appears **0 times** in the whole plan output.
- **Missing declared var is a hard error before any state**: unsetting the var
  gives `Error: secret \`ANTHROPIC_API_KEY\`: declared as a host-environment
  secret, but \`ANTHROPIC_API_KEY\` is not set in hotcell's environment` (and
  the analogous passthrough message); no `.cell/` directory is created.
- **Never written to cell state**: after a real `hotcell run -y`,
  `.cell/default/provisioned_as.json` records only
  `{ "key": "ANTHROPIC_API_KEY", "source": "Secret" }` /
  `{ "key": "PI_PROVIDER", "source": "Passthrough" }` — the value appears **0
  times** anywhere under `.cell/`.
- **README worked example** (now including `env PI_PROVIDER` and
  `secret ANTHROPIC_API_KEY`) run as written with both vars set: literal PATH,
  `PI_PROVIDER anthropic`, `ANTHROPIC_API_KEY <redacted>`, footer present,
  value leaked 0 times.
- **`examples/pi-bootstrap/Cellfile`**: `hotcell run --dry-run -- pi` from the
  example directory with `GOOGLE_API_KEY` exported → `--setenv PI_PROVIDER
  google`, `--setenv GOOGLE_API_KEY <redacted>`, footer present, value leaked 0
  times; the four declared `host-path`s all exist locally. With `GOOGLE_API_KEY`
  unset it errors before any sandbox starts and creates no `.cell/`.

All README examples were also re-run against the rebased binary (quick-start
real run, empty Cellfile, `status`, `--name scratch`, `--dry-run -- pi`,
`--help`, `run --help`, `destroy`) — all behave as documented.

## Review

verdict: approved
reviewer: reviewer-agent
date: 2026-08-24

Independent verification in a fresh worktree checkout, binary rebuilt with
`cargo build` (bwrap 1.x present). Every acceptance item re-checked by running
commands, not just reading prose.

Examples run and results:

- **Quick start** (`workdir "/work"` Cellfile, `hotcell run -y -- echo hello
  from hotcell`): prints `hello from hotcell`; plan carries `--clearenv` and no
  proxy — confirms the clean-env and offline claims.
- **README worked example** (literal `env PATH=...`, passthrough `env
  PI_PROVIDER`, `secret ANTHROPIC_API_KEY`, `net { allow ... port=443; allow
  ... }`): dry-run with both vars set renders `--setenv PATH ...`, `--setenv
  PI_PROVIDER google`, `--setenv ANTHROPIC_API_KEY <redacted>`, plus the footer
  that the block will not run verbatim. Grep for the raw secret in the plan: 0
  matches.
- **Both env shapes distinguished**: literal shows its value; passthrough and
  secret resolve from hotcell's own env. Verified.
- **Missing var is a hard error before any state**: unset secret →
  `Error: secret \`ANTHROPIC_API_KEY\`: ... not set in hotcell's environment`
  (exit 1); unset passthrough → analogous message. No `.cell/` directory is
  created in either case.
- **Never written to cell state**: after a real `hotcell run -y`,
  `.cell/default/provisioned_as.json` holds only
  `{"key":"ANTHROPIC_API_KEY","source":"Secret"}` and
  `{"key":"PI_PROVIDER","source":"Passthrough"}`; `grep -r` for the secret
  value across all of `.cell/` returns 0 matches.
- **package/seed gone**: `package "nodejs"` and `seed "..." "..."` are rejected
  by the parser (`is not implemented — ... removed from the Cellfile grammar`),
  not silently accepted; and neither appears in the README reference. An unknown
  node (`nett { ... }`) fails with `unknown Cellfile node \`nett\``, matching the
  README's typo-safety claim.
- **Endpoint form**: `allow "host" port=N`; port-optional `allow "host"`
  persists as `"port": null`. No `host:port` string in README or example.
- **pi-bootstrap example**: from the example dir, dry-run with `GOOGLE_API_KEY`
  exported renders `--setenv GOOGLE_API_KEY <redacted>` (raw value leaked 0
  times); without the key it errors cleanly before any sandbox and creates no
  state. Comments now instruct exporting the key and using `secret`, never
  pasting it into the file.
- **Other commands**: `status` (default + `--name scratch`), `destroy`
  (prompts `[y/N]` as README shows; `--force` tears down), `--help`,
  `--version` (0.1.0) all behave as written.

Static checks:

- Prerequisites state Rust 1.95+; `Cargo.toml` `rust-version = "1.95"` agrees.
- No stale v1 grammar tokens in README/example: no `key = value`, `host:port`,
  `host_path`, `provision.type`, `net.allow`, `env.*`, `1.81`, `.ini`, or `=>`.
  The only grep hits for `seed`/`package` are the English verb "seed(s)" and
  "package manager" — not grammar directives.
- No overclaiming: secret claims ("redacted in the plan", "never written to
  cell state", "hard error if unset") are each borne out by the runs above; the
  footer correctly warns the plan will not run verbatim.

Nit (non-blocking, not a change request): the real `hotcell run -y` with a
`net` block failed at proxy startup with `path must be shorter than SUN_LEN` —
this is an artifact of the very long scratchpad path used for the Unix socket,
not a documentation defect; the plan, env resolution, and state write all
completed correctly before it.
