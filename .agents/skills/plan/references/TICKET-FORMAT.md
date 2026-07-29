# Ticket format

## Filenames

```
.plan/
  epics/   <NN>-<slug>.md
  stories/ <NN>-<slug>.md
  tasks/   <NN>-<slug>.md
```

- **`NN`** — a 2-digit sort-hint prefix, monotonic *within each kind's
  directory* (`epics/`, `stories/`, `tasks/` each count independently). It
  gives stable `ls` / `git log` ordering. It is **not** the identity.
- **`slug`** — the human handle: short, kebab-case, no spaces, no slashes.
  Example: `http-connect-proxy`. The slug is the identity; frontmatter `id`
  repeats it for grep.
- Uniqueness is guaranteed by the tech lead being the sole creator (it can
  just `ls` the directory).

## Frontmatter (YAML)

```yaml
---
id: http-connect-proxy            # matches the filename slug
kind: task                        # epic | story | task
parent: network-firewall          # parent slug; ABSENT for epics
title: Implement HTTP CONNECT allowlist proxy
status: todo                      # todo | in_progress | review | done | blocked
assignee: null                    # agent id when claimed; else null
created: 2025-07-29
updated: 2025-07-29
tags: [firewall, v1]              # optional
depends_on: []                    # sibling slugs (same parent) that must be done first
---
```

### Fields

| Field | Required | Notes |
|---|---|---|
| `id` | yes | The slug; matches filename without `.md`. |
| `kind` | yes | `epic`, `story`, or `task`. |
| `parent` | stories & tasks | The parent's slug. Omit entirely for epics. |
| `title` | yes | Human-readable; may differ from slug. |
| `status` | yes | `todo` · `in_progress` · `review` · `done` · `blocked`. |
| `assignee` | no | Agent id when claimed; `null` otherwise. |
| `created` | yes | `YYYY-MM-DD`. |
| `updated` | yes | `YYYY-MM-DD`; bump on any edit. |
| `tags` | no | Free-form list. |
| `depends_on` | no | List of sibling slugs (same parent) that must be `done` before this ticket is dispatchable. Enforced by `claim.sh`; shown as `BLOCKED-BY` on the board. Prefer this over the `blocked` status for sibling ordering. |

### Status lifecycle

```
todo  -->  in_progress  -->  review  -->  done
                |               |              ^
                +--> blocked <--+              |
                       |                       |
                       +------ (re-dispatch) --+
```

- `todo` — created on trunk, not yet dispatched.
- `in_progress` — a worker has claimed it (worktree branched). Also set by a
  reviewer who requests changes (back from `review`).
- `review` — worker has self-validated against `## Acceptance` and recorded
  `## Validation`; awaiting independent review.
- `done` — an independent reviewer approved (`## Review` verdict: approved)
  and the tech lead merged. Only ever appears on trunk after `merge-task.sh`.
- `blocked` — cannot proceed for a reason a dependency can't express (external
  decision, etc.); reason in `## Notes`. Prefer `depends_on` for sibling
  ordering.

**`review` → `done` is never self-served.** A worker sets `review`; a
reviewer's approved verdict + the tech lead's merge set `done`. The scripts
enforce this: `merge-task.sh` refuses a merge without `status: review` and an
approved `## Review` verdict.

## Body

Free-form markdown. Sections grow through the lifecycle:

```markdown
## Goal
One or two sentences on the desired outcome.

## Context
Links to the parent story and relevant code paths / files. Anything a worker
needs to orient.

## Acceptance
- [ ] concrete, checkable criteria
```
The worker adds, when self-validating:
```markdown
## Validation
- 2025-07-29 <who>: ran `cargo test --release firewall` -> 4 passed
- 2025-07-29 <who>: manually ran `examples/pi-bootstrap` against Gemini -> ok
```
The reviewer adds, after independently re-checking:
```markdown
## Review
verdict: approved          # or: changes-requested
reviewer: <agent id>
date: 2025-07-29
<what was re-checked and the result>
```

Epics and stories may omit `Acceptance`/`Validation`/`Review` (those live on
tasks) and instead carry a `## Scope` / `## Out of scope` section. Stories may
use `depends_on` for cross-story ordering.

### Review verdict format (machine-checked)

`merge-task.sh` parses the `## Review` section for a `verdict:` line. Use
exactly:
```markdown
verdict: approved
```
or
```markdown
verdict: changes-requested
```
If a second review round occurs, append a new `## Review` block dated beneath
the first (do not delete history); `merge-task.sh` reads the *last* `## Review`
block's verdict.

## Examples

See [../templates/](../templates/) for starter files of each kind.
