# The plan process

## Why this design

The goal is a backlog system where **two agents working two different tickets
never edit the same file**. That property lets any number of workers run in
parallel worktrees without coordination overhead, and it lets merges be
trivial (no plan-bookkeeping conflicts).

Three rules deliver it:

1. **One file per ticket.** All mutable state for a ticket lives in that
   ticket's file.
2. **Downward is stored in the child.** A task names its parent story in
   frontmatter; a story names its parent epic. The parent never records child
   state. So updating a child never touches its parent.
3. **Roll-up is derived, never stored.** A story's progress is computed by
   scanning its child task files on trunk; an epic's by scanning its stories.
   Nobody edits a parent to reflect child progress.

Combined with worktrees, rule 2 means each task branch modifies exactly one
`.plan/` file (its own) plus code — so rebasing or merging alongside other
task branches is conflict-free in `.plan/`. (Code conflicts between two tasks
editing the same source file can still happen — and should: that signals real
overlap between tasks, not a bookkeeping failure to paper over.)

## Trunk is the board

Trunk (default `main`; `PLAN_TRUNK` to override) is the single source of truth
for what tickets exist and their merged statuses. A worktree branch only sees
tickets that existed when it was cut, so workers must read the board from
trunk, not their checkout. `scripts/board.sh` does this read-only via
`git show trunk:.plan/...` — no checkout, no merge, always current.

## The tech lead is the single writer to the backlog

The tech lead is a **foreground** agent run in a turn-taking session with the
developer. It is the only agent that:

- creates epic, story, and task files (so ID/slug allocation is sequential —
  no creation race is possible),
- edits epic and story files (their definitions; rare),
- merges returned task branches into trunk,
- regenerates board snapshots.

It does **not** implement tasks. Workers implement; the tech lead plans and
integrates.

Because only one planning session is live at a time and it commits all new
tickets to trunk *before* any worktree branches off, workers always branch
from a backlog that already contains their task.

## Workflow

### Planning (tech lead + developer, on trunk)

1. Read the board: `./scripts/board.sh`.
2. Create tickets with `scripts/new-ticket.sh` (allocates the sort-hint prefix
   and writes frontmatter from a template), then fill the body and commit.
3. Decide which tasks to dispatch now; create worktrees with
   `scripts/claim.sh` and hand each to a worker.

### Execution (worker, in a worktree)

1. Work in the assigned worktree. `claim.sh` already set the task to
   `in_progress` and committed that flip on the task branch.
2. Read the task file and its parent story for context.
3. Edit **only** your task file (`.plan/tasks/<slug>.md`) and code. Update the
   task's `## Notes` as a log. Never edit any other `.plan/` file.
4. On completion set `status: review` (awaiting tech-lead merge) or `done`,
   commit, and hand the branch back.

### Integration (tech lead, on trunk)

1. `scripts/merge-task.sh <slug>` merges the task branch into trunk and
   removes the worktree. Conflict-free in `.plan/` by construction.
2. Re-branch the next round of workers from the updated trunk so they see the
   newly-merged state and any tickets the tech lead added.
3. Optionally regenerate/commit a `board.md` snapshot — a *view*, never the
   source of truth.

## Concurrency notes

- **Claiming a task** = branching a worktree. No locks, no marker races: two
  agents cannot both write the same task file because they're in separate
  checkouts on separate branches. `status: in_progress` is a *record* of
  intent, not a coordination primitive.
- **Ticket creation** is racy only if workers create tickets. They don't —
  only the tech lead does, in a single sequential session. This is why
  creation is a trunk-side, singly-threaded process.
- **Reading the board** is always safe: `git show` against trunk is read-only
  and never blocks writers.

## Edge cases

- **A worker discovers missing work.** It does *not* create a ticket. It
  records the gap in its task's `## Notes` for the tech lead to triage in the
  next planning session.
- **A task needs to split.** The worker finishes or pauses, hands back the
  branch; the tech lead creates the new sub-tasks on trunk and re-dispatches.
- **Two tasks touch the same code.** Let them conflict at merge time — that's
  a real signal the tasks overlap. Resolve by sequencing (merge one, rebase
  the other) or by re-splitting the work in planning.
- **Renaming a story/epic.** Because the parent link is in frontmatter (not
  the filename or a path), renaming a story's file only requires updating the
  `parent:` field in its child tasks — a small, well-contained edit the tech
  lead does on trunk. Children's filenames are independent of the parent's.
- **No trunk yet / fresh repo.** `scripts/new-ticket.sh` writes to the working
  tree; commit the `.plan/` files to trunk before any `claim.sh` so workers
  branch from a backlog that exists.

## What this scheme deliberately does not have

- **No central `index.md` / `board.md` as source of truth.** A board file is a
  derived view; if you commit one, treat it as a snapshot, regenerable from
  the ticket files.
- **No lock files or claim markers.** The branch is the claim.
- **No hierarchical filenames** (`E01/S02/T03`). Flat slugs per kind keep
  renames cheap and `git log` readable; the parent link lives in frontmatter.
- **No worker-created tickets.** Creation is the tech lead's job, which is
  what makes allocation race-free.
