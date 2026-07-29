#!/usr/bin/env bash
# Merge a completed task branch back into trunk and remove its worktree.
#
# Usage: merge-task.sh <task-slug> [worktree-path] [trunk]
# Env:   PLAN_TRUNK (default main)
#
# Checks out trunk, merges plan/<slug> (--no-ff), removes the worktree, and
# deletes the merged branch. Run from the main checkout (tech lead).
set -euo pipefail

slug="${1:?task slug required}"
wt="${2:-../wt-$slug}"
trunk="${3:-${PLAN_TRUNK:-main}}"
branch="plan/$slug"

git rev-parse --verify -q "$branch" >/dev/null || {
  echo "no such branch: $branch" >&2; exit 1
}

git checkout "$trunk"
git merge --no-ff "$branch" -m "plan: merge $slug"

git worktree remove "$wt" 2>/dev/null || true
git branch -d "$branch" 2>/dev/null || true

echo "merged $branch into $trunk"
