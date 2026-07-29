---
name: plan
description: Trunk-based planning and backlog management for multi-agent development. A tech-lead agent maintains epics, stories, and tasks as one file per ticket on trunk; worker agents implement one task each in a dedicated git worktree branch; a review agent independently verifies completion before merge. Use when planning work, maintaining a backlog, splitting work into tickets, coordinating parallel agents, reviewing a completed task, or picking up a task to implement.
---

# Plan — trunk-based backlog for multi-agent work

This skill coordinates planning, execution, and review across three agent
roles. It is project-agnostic: the only project-specific data is a
git-tracked `.plan/` directory it creates in the repo root.

## Roles

- **Tech lead** — a *foreground* agent run in close coordination with the
  developer. It is the **single writer** to the backlog: it creates and edits
  epic/story/task files on trunk, splits and reprioritizes work, dispatches
  workers and reviewers, merges approved task branches into trunk, and
  regenerates the board. It does **not** implement or review tasks. Because
  planning is a single sequential session with the developer, ticket creation
  has no race.
- **Worker** — an agent that implements **one task** in a dedicated git
  worktree branched off trunk. It edits only its own task file plus code,
  self-validates against the task's `## Acceptance` criteria, records a
  `## Validation` section, and sets `status: review` when it believes the
  task is complete. It does **not** mark its own task `done`.
- **Reviewer** — a *fresh-context*, independent agent that verifies a
  `review`-state task. It reads the task + acceptance criteria, reads the
  diff, and **runs the acceptance checks itself** in the worktree. It edits
  **only the task file** (never code): it adds a `## Review` section with
  `verdict: approved` or `verdict: changes-requested`. On approval the tech
  lead merges; on changes-requested the task returns to the worker.

`review` → `done` is **never** on the honor system: it requires an independent
reviewer's approved verdict, which `merge-task.sh` checks before merging.

## The scheme in one paragraph

One file per ticket under `.plan/{epics,stories,tasks}/`. Filenames are
descriptive slugs with a 2-digit sort-hint prefix (`01-http-connect-proxy.md`).
Relationships point **downward is stored in the child**: a task's frontmatter
names its `parent` story and its `depends_on` siblings; a story names its
`parent` epic. **No agent ever edits a parent to record child progress** —
roll-up is *derived* by scanning child files on trunk at read time. There is
no central mutable index or board file that gets rewritten on every change.
Claiming a task = branching a worktree; the branch *is* the claim, so no
locks are needed.

## Trunk vs. worktree

- **Trunk** (default `main`; override with `PLAN_TRUNK`) holds the backlog and
  is the single source of truth for what tickets exist and their merged
  statuses.
- A **worktree branch** (`plan/<task-slug>`) carries one task's in-flight
  edits — its own task file + the code that implements it — and merges back as
  a unit (PR = task). Two concurrent task branches never conflict in `.plan/`
  because each touches only its own ticket file.

Read the board from **trunk plus open branches**, not just your worktree
checkout. `scripts/board.sh` reads trunk via `git show` for the backlog and
scans `plan/*` branches for in-flight status (including `review`-ready), so
review-ready work is visible before merge — no checkout required.

## Tech-lead workflow

1. With the developer, plan the work. Read the current board:
   ```bash
   ./scripts/board.sh                       # backlog (trunk) + in-flight (branches)
   ```
2. Create tickets (epics/stories on trunk; tasks under a story, with
   `depends_on` set for sibling ordering). Use the helper so slug/prefix
   allocation is consistent:
   ```bash
   ./scripts/new-ticket.sh epic   v1-ship-self-hosted    "Ship v1 self-hostable hotcell"
   ./scripts/new-ticket.sh story  network-firewall       "Network firewall"  v1-ship-self-hosted
   ./scripts/new-ticket.sh task   http-connect-proxy     "HTTP CONNECT allowlist proxy"  network-firewall
   ```
   Then fill the body (Goal / Context / Acceptance / Notes), edit
   `depends_on:` in frontmatter for sibling dependencies, and `git commit`.
3. Dispatch workers — one per **ready** task (deps all `done`) — each in its
   own worktree:
   ```bash
   ./scripts/claim.sh http-connect-proxy   # creates ../wt-http-connect-proxy on plan/http-connect-proxy; refuses if deps unmet
   ```
   Hand the worktree path to the worker agent.
4. When the board shows a task `review` on its branch, dispatch a reviewer:
   ```bash
   ./scripts/review.sh http-connect-proxy   # prints branch, worktree, acceptance, diff
   ```
   Hand that to a fresh-context review agent.
5. When the reviewer records `verdict: approved`, merge:
   ```bash
   ./scripts/merge-task.sh http-connect-proxy   # checks status=review + approved verdict; merges; flips to done
   ```
   On a merge conflict it aborts and prints rebase guidance for the worker.
   Other in-flight task branches merge independently and conflict-free in
   `.plan/`.

See [references/PROCESS.md](references/PROCESS.md) for the full process,
concurrency reasoning, review/validation detail, and edge cases.

## Worker workflow

1. Start in your assigned worktree (path given by the tech lead). The task
   file is already flipped to `in_progress` by `claim.sh`.
2. Read your task file (`.plan/tasks/<NN>-<slug>.md`) and its parent story for
   context. Note `depends_on` — those siblings are `done` (claim.sh checked).
3. **Edit only your task file and code.** Do not edit any other `.plan/` file
   — not the parent story, not siblings.
4. Implement. Keep task-file `## Notes` updated as a log.
5. **Validate before declaring review.** For each item in `## Acceptance`,
   verify it yourself (run the tests, check the behavior). Append a
   `## Validation` section recording what you checked — commands run and their
   results. Only when every acceptance criterion is met:
   - set `status: review`,
   - bump `updated:`,
   - commit, and hand the branch back to the tech lead.
6. Do not create new tickets, and do not set `status: done` — that is the
   reviewer + tech lead's job. If you discover missing work, note it in
   `## Notes` for the tech lead to triage.

## Reviewer workflow

1. You run in **fresh context** in the task's worktree (path from
   `scripts/review.sh`). You are independent of the worker; do not trust the
   worker's self-validation — re-check.
2. Read the task file's `## Acceptance` and `## Validation`, the parent story,
   and the diff (`scripts/review.sh` prints it). **Run the acceptance checks
   yourself** in the worktree.
3. **Edit only the task file** — never code. Add a `## Review` section:
   ```markdown
   ## Review
   verdict: approved          # or: changes-requested
   reviewer: <your id>
   date: 2025-07-29
   <what you checked and the result>
   ```
4. If `approved`: leave `status: review`, commit, hand back to the tech lead
   to merge.
5. If `changes-requested`: set `verdict: changes-requested`, **flip
   `status: in_progress`**, record concretely what failed, commit, and hand
   back. The tech lead re-dispatches the worker.

## Ticket format

Descriptive slug filenames, YAML frontmatter + markdown body. Frontmatter
includes `depends_on` (sibling slugs that must be `done` before this task is
dispatchable). The body grows through the lifecycle: `## Acceptance` (created
by tech lead) → `## Validation` (worker) → `## Review` (reviewer). See
[references/TICKET-FORMAT.md](references/TICKET-FORMAT.md) for the full schema,
slug rules, status lifecycle, and review format; [templates/](templates/) has
starter files.

## Scripts

| Script | Who | Purpose |
|---|---|---|
| `scripts/board.sh` | all | Read-only board: backlog from trunk + in-flight/review-ready from `plan/*` branches |
| `scripts/new-ticket.sh` | tech lead | Scaffold a ticket file with next sort-hint + slug |
| `scripts/claim.sh` | tech lead | Create a worktree branch for a task; flips to `in_progress`; refuses if `depends_on` unmet |
| `scripts/review.sh` | tech lead/reviewer | Brief a reviewer: branch, worktree, acceptance criteria, diff vs trunk |
| `scripts/merge-task.sh` | tech lead | Merge an approved task (`status: review` + `verdict: approved`); flips to `done`; handles conflicts with guidance |

All scripts honor `PLAN_TRUNK` (default `main`) and `PLAN_DIR` (default
`.plan`) env vars, so the scheme works in any repo without editing the skill.

## Extracting this skill

This skill is self-contained and project-agnostic. To use it in another
project, copy `.agents/skills/plan/` into that repo's `.agents/skills/` (or
`~/.agents/skills/` for global use). The `.plan/` directory it produces is the
only project-specific data.
