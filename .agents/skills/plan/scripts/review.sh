#!/usr/bin/env bash
# Brief a review agent for a task in the review state. Read-only.
#
# Usage: review.sh <task-slug> [trunk]
# Env:   PLAN_TRUNK (default main), PLAN_DIR (default .plan)
#
# Prints the branch, worktree path (if any), task file path, the task's
# ## Acceptance and ## Validation sections, and the full diff vs trunk.
# Intended to be handed to a fresh-context review agent.
set -euo pipefail

slug="${1:?task slug required}"
trunk="${2:-${PLAN_TRUNK:-main}}"
plan="${PLAN_DIR:-.plan}"
branch="plan/$slug"

git rev-parse --verify -q "$branch" >/dev/null || {
  echo "no such branch: $branch" >&2
  exit 1
}

f=$(git ls-tree -r --name-only "$branch" -- "$plan/tasks" 2>/dev/null \
    | grep -E -- "/[0-9]+-${slug}\.md$" | head -n1 || true)
[[ -z "$f" ]] && { echo "no task file for '$slug' on $branch" >&2; exit 1; }

# Locate the worktree for this branch, if any.
wt=$(git worktree list --porcelain 2>/dev/null \
     | awk -v b="refs/heads/$branch" '
         $1=="worktree"{w=$2}
         $1=="branch" && $2==b {print w; exit}
       ' | head -n1)

echo "branch:    $branch"
echo "task:      $f"
echo "worktree:  ${wt:-(none — checkout $branch to review)}"
echo
echo "--- acceptance ---"
git show "$branch:$f" | awk '/^## Acceptance/{a=1;next} /^## /{a=0} a{print}'
echo
echo "--- validation (worker self-check) ---"
git show "$branch:$f" | awk '/^## Validation/{a=1;next} /^## /{a=0} a{print}' \
  | sed '/^[[:space:]]*$/d' || true
echo
echo "--- diff vs $trunk ---"
git diff "$trunk..$branch"
echo
cat <<'GUIDANCE'
--- reviewer guidance ---
You are an independent reviewer in fresh context. Do NOT trust the worker's
self-validation; re-check everything yourself.

1. Read ## Acceptance above and the diff.
2. In the worktree, RUN the acceptance checks yourself (tests, commands,
   manual verification).
3. Edit ONLY the task file (never code). Add a ## Review section:
       ## Review
       verdict: approved          # or: changes-requested
       reviewer: <your id>
       date: <YYYY-MM-DD>
       <what you re-checked and the result>
4. If approved: leave status: review, commit, hand back to the tech lead.
5. If changes-requested: also flip status: in_progress, record concretely what
   failed, commit, hand back. The worker will be re-dispatched.
GUIDANCE
