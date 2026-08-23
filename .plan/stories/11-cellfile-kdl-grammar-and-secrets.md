---
id: cellfile-kdl-grammar-and-secrets
kind: story
parent: v1-self-hostable-hotcell
title: KDL Cellfile grammar with env passthrough and secrets
status: todo
assignee: null
created: 2026-08-23
updated: 2026-08-23
tags: [cellfile, secrets, grammar]
---

## Goal

Replace the hand-rolled Cellfile format with KDL v2, and add declared
passthrough and secret environment variables so an API key never has to be
written into the Cellfile, printed in the execution plan, or persisted to
cell state.

## Context

Two problems meet here.

**Secrets leak.** `examples/pi-bootstrap/Cellfile` instructs the user to put
their provider API key in `env.*`, because the agent runs `--clearenv` and
only declared vars reach it. That key is then rendered verbatim into the
execution plan on every run (including `--dry-run`), so it lands in terminal
scrollback and any captured stderr; and it is serialised into
`.cell/<name>/provisioned_as.json` in plaintext. Because `environment` is part
of `CellDeclaration` and `is_current` compares whole declarations, a resolved
secret value would also become part of cell identity — rotating a key would
force a full re-provision.

**The format is at its limit.** The current parser is line-based `key = value`
with dotted keys, a wildcard `env.` segment, and two ad-hoc value grammars
(`=>` for seeds, `host:port` for endpoints). `split_kv` rejects any line
without `=`, so the directive shape the passthrough feature wants
(`secret ANTHROPIC_API_KEY`) cannot parse at all. Policy v2 (globs, CIDRs,
warn mode) and L7 rules need structure the flat format cannot carry.

Grammar decisions, settled with the developer 2026-08-23:

- **KDL v2** via the `kdl` crate (6.x). Verified empirically: v2 accepts bare
  identifiers as arguments, so `secret ANTHROPIC_API_KEY` parses, and the two
  `env` shapes distinguish themselves — props-and-no-args is a literal,
  args-and-no-props is a passthrough. `knus` was rejected: it offers derive
  macros but is KDL **v1**, which rejects bare identifier arguments
  ("identifiers cannot be used as arguments") and would force
  `secret "ANTHROPIC_API_KEY"` — the quoting the directive shape exists to
  avoid.
- **MSRV rises 1.81 → 1.95** (the `kdl` crate's floor). Accepted.
- `package` and `seed` are **dropped** from the grammar rather than ported.
  They parse today and no provisioner reads them; see `packages-seeds-semantics`.
- Endpoints **split** into host plus an optional `port` property, retiring the
  `rsplit_once(':')` mini-grammar and the IPv6 bug it carries.
- **No `--secret` CLI flag.** A CLI-injected var would make the running cell's
  declaration differ from the file it came from — the same one-shot-deviation
  problem as the spec's `provisioner_override`.

Target grammar:

```kdl
workdir "/work"

provision "shell" script="./provision.sh" {
    host-path "~/.nvm/versions/node"
}

env PATH="/opt/pi/bin:/usr/bin:/bin"
env PI_PROVIDER              // passthrough from the host env
secret ANTHROPIC_API_KEY     // passthrough, redacted in the plan

net {
    allow "api.anthropic.com" port=443
}
```

## Tasks

- `replace-cellfile-parser-with-kdl`
- `resolve-env-passthrough-and-secrets`
- `redact-secrets-in-execution-plan`
- `document-kdl-cellfile-format`

## Notes

- 2026-08-23 created from a design session. Every grammar decision above was
  made explicitly with the developer; a worker should not re-litigate them.
- **Conflicts with `extend-network-policy-types-and-persistence`**, which
  assumes additive `net.allow = host[:port]` keys and back-compat for existing
  Cellfiles. If this story lands first, that task must be re-planned against
  the KDL grammar. The leader should sequence these deliberately.
- Deliberately excluded: whether `environment` should participate in cell
  identity at all — see `env-changes-should-not-reprovision`.
