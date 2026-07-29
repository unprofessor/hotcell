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

### Status lifecycle

```
todo  -->  in_progress  -->  review  -->  done
                |               |
                +--> blocked <--+
```

- `todo` — created on trunk, not yet dispatched.
- `in_progress` — a worker has claimed it (worktree branched).
- `review` — worker finished, awaiting tech-lead merge.
- `done` — merged to trunk.
- `blocked` — cannot proceed; reason in `## Notes`. The tech lead triages.

## Body

Free-form markdown. Recommended sections:

```markdown
## Goal
One or two sentences on the desired outcome.

## Context
Links to the parent story and relevant code paths / files. Anything a worker
needs to orient.

## Acceptance
- [ ] concrete, checkable criteria

## Notes
- YYYY-MM-DD <who>: <log entry>
```

Epics and stories may omit `Acceptance` (that lives on tasks) and instead carry
a `## Scope` / `## Out of scope` section.

## Examples

See [../templates/](../templates/) for starter files of each kind.
