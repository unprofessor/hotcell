---
name: plan
description: Trunk-based planning and backlog management for multi-agent development. A tech-lead agent maintains epics, stories, and tasks as one file per ticket on trunk; worker agents implement one task each in a dedicated git worktree branch. Use when planning work, maintaining a backlog, splitting work into tickets, coordinating parallel agents, or picking up a task to implement.
---

# Plan — trunk-based backlog for multi-agent work

This skill coordinates planning and execution across a tech-lead agent and one
or more worker agents. It is project-agnostic: the only project-specific data
is a git-tracked `.plan/` directory it creates in the repo root.

## Roles

- **Tech lead** — a *foreground* agent run in close coordination with the
  developer. It is the **single writer** to the backlog: it creates and edits
  epic/story/task files on trunk, splits and reprioritizes work, reviews and
  merges returned task branches, and regenerates the board. It does **not**
  implement tasks. Because planning is a single sequential session with the
  developer, ticket creation has no race.
- **Worker** — an agent that implements **one task** in a dedicated git
  worktree branched off trunk. It edits only its own task file plus code,
  sets the task `in_progress`, and returns the branch for the tech lead to
  merge.

## The scheme in one paragraph

One file per ticket under `.plan/{epics,stories,tasks}/`. Filenames are
descriptive slugs with a 2-digit sort-hint prefix (`01-http-connect-proxy.md`).
Relationships point **downward is stored in the child**: a task's frontmatter
names its `parent` story; a story names its `parent` epic. **No agent ever
edits a parent to record child progress** — roll-up is *derived* by scanning
child files on trunk at read time. There is no central mutable index or board
file that gets rewritten on every change. Claiming a task = branching a
worktree; the branch *is* the claim, so no locks are needed.

## Trunk vs. worktree

- **Trunk** (default `main`; override with `PLAN_TRUNK`) holds the backlog and
  is the single source of truth for what tickets exist and their merged
  statuses.
- A **worktree branch** (`plan/<task-slug>`) carries one task's in-flight edits
  — its own task file + the code that implements it — and merges back as a unit
  (PR = task). Two concurrent task branches never conflict in `.plan/` because
  each touches only its own ticket file.

Read the board from **trunk**, not your worktree checkout (your branch only
sees tickets that existed when it was cut). Use `scripts/board.sh` which reads
via `git show` — no checkout, always current.

## Tech-lead workflow

1. With the developer, plan the work. Read the current board:
   ```bash
   ./scripts/board.sh                       # lists all tickets on trunk
   ```
2. Create tickets (epics/stories on trunk; tasks under a story). Use the
   helper so slug/prefix allocation is consistent:
   ```bash
   ./scripts/new-ticket.sh epic   v1-ship-self-hosted    "Ship v1 self-hostable hotcell"
   ./scripts/new-ticket.sh story  network-firewall       "Network firewall"  v1-ship-self-hosted
   ./scripts/new-ticket.sh task   http-connect-proxy     "HTTP CONNECT allowlist proxy"  network-firewall
   ```
   Then fill the body (Goal / Context / Acceptance / Notes) and `git commit`.
3. Dispatch workers — one per task — each in its own worktree:
   ```bash
   ./scripts/claim.sh http-connect-proxy   # creates ../wt-http-connect-proxy on plan/http-connect-proxy
   ```
   Hand the worktree path to the worker agent.
4. When a worker returns, merge and clean up:
   ```bash
   ./scripts/merge-task.sh http-connect-proxy
   ```
   Other in-flight task branches merge independently and conflict-free in
   `.plan/`.
5. Regenerate a human board view if desired (`scripts/board.sh` output, or
   commit a `board.md` snapshot — a *view*, never the source of truth).

See [references/PROCESS.md](references/PROCESS.md) for the full process,
concurrency reasoning, and edge cases.

## Worker workflow

1. Start in your assigned worktree (path given by the tech lead). The task
   file is already flipped to `in_progress` by `claim.sh`.
2. Read your task file (`.plan/tasks/<NN>-<slug>.md`) and its parent story for
   context. **Edit only your task file and code.** Do not edit any other
   `.plan/` file — not the parent story, not siblings.
3. Implement. Keep task-file `## Notes` updated as a log. When done, set
   `status: review` (or `done`), commit, and hand the branch back to the tech
   lead for merge.
4. Do not create new tickets. If you discover missing work, note it in your
   task's `## Notes` for the tech lead to triage.

## Ticket format

Descriptive slug filenames, YAML frontmatter + markdown body. See
[references/TICKET-FORMAT.md](references/TICKET-FORMAT.md) for the full schema,
slug rules, and examples; [templates/](templates/) has starter files.

## Scripts

| Script | Who | Purpose |
|---|---|---|
| `scripts/board.sh` | both | Read-only board view from trunk via `git show` |
| `scripts/new-ticket.sh` | tech lead | Scaffold a ticket file with next sort-hint + slug |
| `scripts/claim.sh` | tech lead | Create a worktree branch for a task, flip to `in_progress` |
| `scripts/merge-task.sh` | tech lead | Merge a returned task branch into trunk, drop the worktree |

All scripts honor `PLAN_TRUNK` (default `main`) and `PLAN_DIR` (default
`.plan`) env vars, so the scheme works in any repo without editing the skill.

## Extracting this skill

This skill is self-contained and project-agnostic. To use it in another
project, copy `.agents/skills/plan/` into that repo's `.agents/skills/` (or
`~/.agents/skills/` for global use). The `.plan/` directory it produces is the
only project-specific data.
