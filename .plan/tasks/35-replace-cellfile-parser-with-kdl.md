---
id: replace-cellfile-parser-with-kdl
aliases: [replace-cellfile-parser-with-kdl]
kind: task
parent: cellfile-kdl-grammar-and-secrets
title: Replace the Cellfile parser with KDL v2
status: done
assignee: worker-1
created: 2026-08-23
updated: 2026-08-23
tags: [cellfile, grammar, parser]
depends_on: []
---

## Goal

Parse Cellfiles as KDL v2 documents, carry the new env source model in
`CellDeclaration`, and migrate every Cellfile in the repo. Must land green:
the tests cannot compile against the old format, so their migration belongs
here.

## Context

`src/cellfile.rs` hand-rolls a line format in ~200 lines (`parse`,
`split_kv`, `strip_comment`, `parse_endpoint`, `parse_file_mapping`). Replace
that with a walk over `kdl::KdlDocument`. Grammar and rationale are fixed in
the parent story — do not re-litigate them.

The env model changes shape, so `provisioned_as.json` records written by older
versions will not load. That is accepted; no migration path is required for
cell state.

Cellfiles to migrate: `examples/pi-bootstrap/Cellfile`, plus inline Cellfiles
written by 9 integration tests (`tests/run_*.rs`, `tests/destroy_*.rs`).

## Acceptance

- [x] `kdl` 6.x is a dependency; `Cargo.toml` declares `rust-version = "1.95"`
  and CI still passes on stable and beta.
- [x] `CellDeclaration.environment` holds `EnvVar { key, source }` where
  `source` is `Literal(String) | Passthrough | Secret`. No resolved value for
  a passthrough or secret is ever stored in the declaration.
- [x] `env NAME="value"` parses as `Literal`, `env NAME` as `Passthrough`,
  `secret NAME` as `Secret`. A node carrying both an argument and a property
  is a source-located error.
- [x] `net { allow "host" port=443 }` parses; `port` is optional and absent
  means any port. IPv6 literals parse correctly — the `rsplit_once(':')` bug
  is gone, with a regression test.
- [x] `provision "shell" script="..."` with `host-path` children parses;
  `provision "none"` and an empty document both yield the documented defaults
  (no-op provisioner, offline, workdir `/work`).
- [x] `package` and `seed` are not in the grammar. Encountering either yields
  an error naming the node and stating that it is not implemented.
- [x] Unknown nodes remain errors (fail-closed, as today).
- [x] Parse errors carry `path:line:col` and are `anyhow` values — kdl's
  miette diagnostic is flattened, not surfaced, and `miette`'s fancy feature
  is not added.
- [x] A file that still uses the old `key = value` shape produces a migration
  hint rather than a bare KDL syntax error.
- [x] `examples/pi-bootstrap/Cellfile` and all test Cellfiles are migrated;
  `cargo test` and `cargo clippy --all-targets` pass.

## Validation

All commands run from the task worktree, 2026-08-23.

### Gates

```
$ cargo fmt --all -- --check
fmt: clean

$ RUSTFLAGS="-D warnings" cargo clippy --all-targets --all-features
    Checking hotcell v0.1.0 (.../worktrees/agent-acfda6dd6153344c0)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 10.01s

$ RUSTFLAGS="-D warnings" cargo test --all-targets --all-features
running 21 tests   test result: ok. 21 passed   (unit: src/, 12 of them cellfile)
running 6  tests   test result: ok. 6 passed    (destroy_confirmation)
running 3  tests   test result: ok. 3 passed    (run_cell_override)
running 5  tests   test result: ok. 5 passed    (run_cellfile_grammar, new)
running 9  tests   test result: ok. 9 passed    (run_dry_run)
running 4  tests   test result: ok. 4 passed    (run_file_override)
running 1  test    test result: ok. 1 passed    (run_firewall_integration)
running 4  tests   test result: ok. 4 passed    (run_global_flags)
running 3  tests   test result: ok. 3 passed    (run_loopback_network)
running 2  tests   test result: ok. 2 passed    (run_minimal)
running 4  tests   test result: ok. 4 passed    (run_risk_profiles)
running 5  tests   test result: ok. 5 passed    (run_script_provisioner)
```

67 tests pass, 0 fail (55 before this task: the cellfile unit tests went 5 ->
12, and `tests/run_cellfile_grammar.rs` is new with 5). bwrap is installed on this
machine, so the sandboxing tests really ran.

### MSRV

`kdl-6.7.1/Cargo.toml` declares `rust-version = "1.95"`; `Cargo.toml` now
declares the same. Verified by building the whole crate on that exact
toolchain, not just by reading the manifest:

```
$ rustup toolchain install 1.95.0 --profile minimal
  1.95.0-x86_64-unknown-linux-gnu installed - rustc 1.95.0 (59807616e 2026-04-14)

$ cargo +1.95.0 build --all-targets
   Compiling kdl v6.7.1
   Compiling hotcell v0.1.0 (.../worktrees/agent-acfda6dd6153344c0)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 13.34s
```

CI's matrix is stable + beta (both far above 1.95) and has no pinned-MSRV
job, so no workflow change was needed. `miette` arrives only transitively
via `kdl`, without its `fancy` feature: `grep -n miette Cargo.toml` finds
nothing.

### Behaviour, observed through the CLI

`hotcell run --dry-run -- /bin/true` against a Cellfile with each shape.
Output is verbatim (temp dir path elided in the same way each time):

```
Cellfile: provision.type = shell / workdir = /work        [the old format]
Error: <dir>/Cellfile:1:16: Expected end of document
<dir>/Cellfile:2:9: Expected end of document
<dir>/Cellfile:2:11: Found invalid node name
hint: line 1 (`provision.type = shell`) looks like the old `key = value`
Cellfile format. Cellfiles are now KDL v2: `provision "shell"
script="./provision.sh"`, `workdir "/work"`, `env NAME="value"`,
`net { allow "host" port=443 }`, and comments start with `//`.
exit: 1

Cellfile: secret ANTHROPIC_API_KEY
Error: secret `ANTHROPIC_API_KEY`: secrets are not yet implemented —
declare it as `env ANTHROPIC_API_KEY="value"` for now
exit: 1

Cellfile: package "node"
Error: <dir>/Cellfile:1:1: `package` is not implemented — it was removed
from the Cellfile grammar because no provisioner acted on it; have the
provision script do this work instead
exit: 1

Cellfile: env FOO="a" BAR
Error: <dir>/Cellfile:1:1: `env` takes either a literal `NAME="value"` or a
bare `NAME` to pass through from the host environment, not both
exit: 1

Cellfile: net { allow "2001:db8::1" port=8443 }
provisioning: none (no bwrap during provisioning)
=== agent bwrap ===
bwrap \
  --bind <dir>/.cell/default/fs / \
  ...
exit: 0        [IPv6 host accepted; the v1 rsplit_once(':') would have
                split it into host "2001:db8:" + a bad port]
```

The migrated `examples/pi-bootstrap/Cellfile` was planned end to end:

```
$ cargo run -q -- --file examples/pi-bootstrap/Cellfile run --dry-run -- pi
=== provisioning bwrap ===
  ...
  --ro-bind /home/exfed/.nvm/versions/node /hotcell/host/home/exfed/.nvm/versions/node
  --ro-bind /home/exfed/.pi /hotcell/host/home/exfed/.pi
  --ro-bind /home/exfed/.agents/skills /hotcell/host/home/exfed/.agents/skills
  --ro-bind /home/exfed/.gitconfig /hotcell/host/home/exfed/.gitconfig
  ...
=== agent bwrap ===
  --clearenv
  --setenv PATH /opt/pi/bin:/opt/node/bin:/usr/bin:/bin
  --setenv HOME /home/agent
  --setenv PI_PROVIDER google
```

All four `host-path` children, both literal `env` vars and the `net` entry
survive the migration with the same effect as the v1 file.

### Per acceptance box

- **kdl 6.x + rust-version** — `cargo add kdl@6` resolved 6.7.1; MSRV build
  above.
- **`EnvVar { key, source }`** — `src/cellfile.rs`; `EnvSource::Passthrough`
  and `::Secret` are unit variants, so there is nowhere to put a resolved
  value. Covered by `cellfile::tests::parses_all_nodes`, which asserts the
  whole `environment` vector by value.
- **three env shapes + both-is-an-error** —
  `cellfile::tests::parses_all_nodes`,
  `::env_node_with_both_an_argument_and_a_property_is_an_error` (asserts the
  `:1:1:` location too), `::env_with_nothing_is_an_error`.
- **`net`/`port`/IPv6** — `cellfile::tests::ipv6_endpoints_keep_their_colons`
  (`::1`, `2001:db8::1` with a port, `fe80::1%eth0`), `::bad_port_is_an_error`
  (65535 overflow and a quoted port), plus the CLI run above.
- **`provision` + defaults** — `cellfile::tests::parses_all_nodes` for the
  populated form, `::provision_none_and_empty_document_agree` asserts
  `provision "none"` and an empty document produce *equal* declarations.
  Effective workdir `/work` for both comes from `plan::DEFAULT_WORKDIR`, as
  before — see Notes.
- **`package`/`seed` error** — `cellfile::tests::package_and_seed_report_...`
  and `run_cellfile_grammar::package_and_seed_report_that_they_are_gone`.
- **unknown nodes fail closed** — `cellfile::tests::unknown_nodes_are_errors`
  covers a top-level node, a node inside `net`, and a misspelled property on
  `provision`; `run_cellfile_grammar::an_unknown_node_is_a_located_error`
  checks it through the CLI with its line:col.
- **`path:line:col` anyhow errors, no miette** — every error above carries
  one; `syntax_error` flattens `KdlError::diagnostics` to text.
- **migration hint** — `cellfile::tests::a_v1_cellfile_gets_a_migration_hint`,
  `::a_v1_hash_comment_gets_a_migration_hint` (a `#` comment alone triggers
  it, since KDL rejects `#`), and the CLI transcript above.
- **migrations** — `examples/pi-bootstrap/Cellfile` and the inline Cellfiles
  in all 9 integration test files; the suite passes.

## Notes

- 2026-08-23 created. Verified during design: KDL v2 accepts bare identifier
  arguments, and args-vs-props cleanly separates the two `env` shapes.
- Resolution of passthrough values is NOT in this task — the parser only
  records which vars are passthrough or secret. See
  `resolve-env-passthrough-and-secrets`.
- 2026-08-23 developer confirmed: **breaking compatibility and eliding
  migration are both fine** — hotcell is still internal. No compat shim, no
  state migration, no dual-format parsing. The old-format hint in the
  acceptance criteria stays only because it is a few lines of error handling
  that the team will hit with its own existing Cellfiles; it is not a
  compatibility path.
- 2026-08-23 worker-1, decisions a reviewer should look at:
  - **Non-literal vars fail the plan.** `src/plan.rs` maps `environment` to
    the agent's `--setenv` list; a `Passthrough` or `Secret` there now raises
    "not yet implemented ... declare it as `env NAME=\"value\"` for now"
    rather than being dropped. Chosen per the task's instruction to prefer a
    clear error; `resolve-env-passthrough-and-secrets` replaces it. Covered
    by `run_cellfile_grammar::passthrough_and_secret_vars_are_not_yet_resolved`,
    which will need updating when that lands.
  - **`packages` and `files` stay on `CellDeclaration`.** No grammar
    populates them, so they are always empty. Removing them would diverge
    from `hotcell.allium`, which still declares both on `CellDeclaration`,
    and whether the capability returns is `packages-seeds-semantics` to
    decide — so the fields (and `FileMapping`) were left in place with a doc
    comment pointing at that story. A reviewer may reasonably want them gone
    instead; that is a spec change, not a parser change.
  - **`workdir` default unchanged.** An empty document still yields
    `workdir: ""` in the declaration, and `plan::DEFAULT_WORKDIR` supplies
    `/work` at plan time, exactly as before. Baking `/work` into the parser
    would have changed the persisted declaration and hence cell identity for
    no gain.
  - **Error messages naming v1 syntax were updated.** `provisioning.rs` told
    users to set `provision.script`, which no longer exists; it now says a
    `script="..."` property on the `provision` node. Two tests asserted on the
    old string and were updated with it. Doc comments in `cell.rs`,
    `isolation.rs` and two test files that named `provision.host_path` /
    `env.*` were likewise re-spelled. The README is deliberately untouched —
    that is `document-kdl-cellfile-format`.
  - **`env NAME` accepts any KDL string argument**, so `env "NAME"` parses
    identically to `env NAME`; only empty names and names containing `=` or
    NUL are rejected. Bare identifiers and quoted strings are the same value
    type in KDL v2, so distinguishing them would mean inspecting source
    formatting.
  - `hotcell.allium` was not touched: `EnvVar` there is `{key, value}`, which
    the new `{key, source}` no longer matches. Updating the spec felt outside
    a parser task, but the divergence is real and someone should own it —
    flagging it rather than silently leaving it.

## Review

Reviewed 2026-08-23 by an independent reviewer on branch
`worktree-agent-acfda6dd6153344c0` (3 commits on `main`). Every check below
was re-run from scratch; none of the worker's numbers were copied.

### Gates

- `cargo fmt --all -- --check` — clean, exit 0.
- `RUSTFLAGS="-D warnings" cargo clippy --all-targets --all-features` — exit
  0, zero warnings. Re-run after `touch src/*.rs tests/*.rs` so it was a real
  re-check, not a fingerprint hit.
- `cargo test --all-targets --all-features` — exit 0. 21 unit tests (12 in
  `cellfile`), 6 + 3 + 5 + 9 + 4 + 1 + 4 + 3 + 2 + 4 + 5 integration = 67
  total, 0 failed, 0 ignored.
- The sandbox tests genuinely sandbox. Confirmed independently rather than
  inferred from timings: `hotcell run -y -- /bin/sh -c 'pwd; echo $GREETING'`
  on a scratch Cellfile printed `/work` and `hi there` from inside a real
  `bwrap --unshare-pid --unshare-net --clearenv` invocation. `/usr/bin/bwrap`
  and `/usr/bin/python3` are both present, so no test short-circuited.

### MSRV

`cargo +1.95.0 build --all-targets` compiled the whole crate and all test
targets, exit 0. `kdl` resolved to 6.7.1, whose manifest declares
`rust-version = "1.95"`; `Cargo.toml` now matches. `.github/workflows/ci.yml`
runs stable + beta only, with no pinned-MSRV job, so no workflow change was
needed. `miette` 7.6.0 arrives only under `kdl`, and its `fancy` feature is
off: `cargo tree -e features` shows only `cfg-if` and `unicode-width` under
it, and `grep -c` for `supports-color`/`owo-colors`/`textwrap`/`terminal_size`
in `Cargo.lock` returns 0.

### The example Cellfile is faithfully migrated — proven, not eyeballed

Built `main`'s binary from `git archive main` into a scratch tree and diffed
the full plan for `examples/pi-bootstrap/Cellfile` against this branch's
binary, with the two source roots normalised:

```
$ diff -u <main plan> <branch plan>
IDENTICAL PLANS
```

Byte-for-byte identical: the same rootfs bind, the same four `--ro-bind`
host-path stagings (`~/.nvm/versions/node`, `~/.pi`, `~/.agents/skills`,
`~/.gitconfig`), the same `--chdir`, and the same agent `--setenv PATH`,
`HOME`, `PI_PROVIDER`. Nothing was gained or lost in the migration.

### IPv6 — the motivating bug is real and is fixed

Verified in both directions. On `main`, `net.allow = 2001:db8::1` persisted as

```
{'allowed_endpoints': [{'host': '2001:db8:', 'port': 1}]}
```

— a silently wrong allowlist entry, exactly the `rsplit_once(':')` failure.
On this branch, `net { allow "2001:db8::1" port=8443; allow "::1" }` persists
to `provisioned_as.json` as `2001:db8::1`/8443 and `::1`/null, colons intact.

### Fail-closed behaviour — probed directly, beyond the test suite

Every one of these was written as a scratch Cellfile and run through
`hotcell run --dry-run`. All are errors with `path:line:col`; none silently
produced a default:

| Input | Result |
| --- | --- |
| `nett { allow "h" }` | `Cellfile:2:1: unknown Cellfile node \`nett\`` |
| `net { deny "h" }` | unknown node inside `net` |
| `provision "shell" scrpit="..."` | unknown property on `provision` |
| `host-path "/tmp" ro=#true` | `host-path` takes no `ro=` property |
| `secret FOO="x"` | `secret` takes no `FOO=` property |
| `workdir "/work" { foo }` | `workdir` takes no child block |
| `net "x" { allow "a" }` | `net` takes no arguments |
| `provision` (no arg) | takes exactly one value, got 0 |
| `workdir 5`, `env FOO=42`, `env FOO=#true` | must be a string, got `…` |
| `port=99999` / `port=0` / `port=-1` / `port="443"` | invalid port, located |
| `env FOO="a" BAR` | "…not both", at `:1:1:` |
| `env` alone / `env ""` | needs a variable / not a usable name |
| `package "node"` / `seed "./a"` | "is not implemented — it was removed…" |
| a full v1 `key = value` file | KDL diagnostics **plus** the migration hint naming line 1 |

The migration hint also fires on a v1 file containing only `net.allow = …`,
only `env.PATH = …`, only `workdir = …`, or a leading `#` comment. Exit code
is 1 in every failing case, and no `.cell` state is created.

### Non-literal env vars (flagged item 1)

The error path is correct and correctly ordered. `build_execution_plan` maps
`environment` and `?`-returns on `Passthrough`/`Secret` at plan.rs:~218,
*before* `provision_control_env` is called at plan.rs:240 — so a
`secret HOME` can never silently fall through to the `/home/agent` default in
`provision_control_env`. Verified end to end: `secret HOME` and
`env PI_PROVIDER` each fail with a named, actionable message, exit 1, and
leave no `.cell` directory behind. `cli.rs:179` is the only caller of the plan
builder, so there is no bypass.

### Test coverage is behavioural, not ceremonial

The 12 cellfile unit tests assert parsed values *by value* (`parses_all_nodes`
compares the whole `environment` vector, `ipv6_endpoints_keep_their_colons`
compares the whole endpoint vector), assert error substrings *together with*
the `:line:col` they must carry (`:1:1:`, `:2:18:`), and
`provision_none_and_empty_document_agree` asserts two declarations are
*equal* rather than spot-checking. The 5 new integration tests go through the
real binary and check stderr plus exit status. All five behaviours the old
unit tests covered are preserved or superseded (v1 `#` comment stripping is
replaced by KDL `//`, exercised in `parses_all_nodes`). These would catch a
regression.

### Non-blocking findings — for follow-up, not for this task

1. `src/provisioning.rs:273` still says "declared at `provision.script`" in a
   doc comment. The same sweep updated `cell.rs` and `isolation.rs`; this one
   was missed. Cosmetic.
2. Duplicate single-valued nodes silently last-win. Two `workdir` nodes yield
   the second; two `provision` nodes yield the second and *silently discard
   the first's `host-path` children*; a duplicated `script=` on one node
   likewise. This matches the v1 parser's last-wins semantics exactly, so it
   is not a regression and is out of scope here — but for a grammar whose
   selling point is failing closed, rejecting a repeated single-valued node
   would be strictly better. Worth a ticket.
3. `env FOO="a" BAR="b"` (two properties, no argument) falls into the
   "…either a literal or a bare NAME, not both" arm, so the message misstates
   what is wrong. Still an error, so fail-closed holds.
4. KDL type annotations are silently ignored: `(ty)workdir "/work"` parses as
   `workdir`. Harmless today; would matter if annotations ever carry meaning.
5. The v1 migration hint prints *after* up to a dozen raw KDL diagnostics.
   Leading with the hint would read better for the team's own files.
6. Flagged item 3 confirmed: `hotcell.allium` still declares
   `EnvVar { key, value }` and references `env.HOME` / `provision.host_path`.
   Real divergence, separately ticketed, not grounds for changes here.
7. Flagged items 2 (`packages`/`files` retained, always empty), 4 (`env "NAME"`
   ≡ `env NAME`) and 5 (README) were checked and judged acceptable: the fields
   are documented as inert and pointed at `packages-seeds-semantics`, KDL v2
   cannot distinguish a bare identifier from a quoted string without
   inspecting source formatting, and the README belongs to
   `document-kdl-cellfile-format`. `find`/`grep` confirm the README is the only
   remaining v1-format documentation in the tree.

### Acceptance

All 11 boxes verified independently. Nothing was left unverified.

verdict: approved
