#!/usr/bin/env bash
# Create a worktree branch for a task and flip it to in_progress.
#
# Usage: claim.sh <task-slug> [worktree-path] [trunk]
# Env:   PLAN_TRUNK (default main), PLAN_DIR (default .plan)
#
# Creates branch plan/<slug> in a new worktree (default ../wt-<slug>) off
# trunk, sets the task's status: in_progress and bumps updated:, and commits
# that flip on the task branch. Prints the worktree path.
set -euo pipefail

slug="${1:?task slug required}"
wt="${2:-../wt-$slug}"
trunk="${3:-${PLAN_TRUNK:-main}}"
plan="${PLAN_DIR:-.plan}"
branch="plan/$slug"

path=$(git ls-tree -r --name-only "$trunk" -- "$plan/tasks" 2>/dev/null \
       | grep -E "/${slug}\.md$" | head -n1 || true)
[[ -z "$path" ]] && { echo "no task file for slug '$slug' on $trunk" >&2; exit 1; }

git worktree add -b "$branch" "$wt" "$trunk"

date="$(date +%F)"
(
  cd "$wt"
  sed -i -E \
    -e "s/^status: .*/status: in_progress/" \
    -e "s/^updated: .*/updated: $date/" \
    "$path"
  git add "$path"
  git commit -m "plan: claim $slug (in_progress)" >/dev/null
)

echo "$wt"
