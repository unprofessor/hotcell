#!/usr/bin/env bash
# Merge a reviewed-and-approved task branch back into trunk.
#
# Usage: merge-task.sh <task-slug> [worktree-path] [trunk]
# Env:   PLAN_TRUNK (default main), PLAN_DIR (default .plan)
#
# Guards:
#   - the task file on the branch must have status: review
#   - the task file must have a ## Review section with verdict: approved
# On success: merges (--no-ff) into trunk, flips the task to status: done on
# trunk (bumping updated:), removes the worktree, deletes the branch.
# On merge conflict: aborts the merge, lists conflicted files, prints rebase
# guidance, and leaves the worktree + branch intact for the worker to rebase.
set -euo pipefail

slug="${1:?task slug required}"
wt="${2:-../wt-$slug}"
trunk="${3:-${PLAN_TRUNK:-main}}"
plan="${PLAN_DIR:-.plan}"
branch="plan/$slug"

git rev-parse --verify -q "$branch" >/dev/null || {
  echo "no such branch: $branch" >&2; exit 1
}

f=$(git ls-tree -r --name-only "$branch" -- "$plan/tasks" 2>/dev/null \
    | grep -E -- "/[0-9]+-${slug}\.md$" | head -n1 || true)
[[ -z "$f" ]] && { echo "no task file for '$slug' on $branch" >&2; exit 1; }

blob=$(git show "$branch:$f")
status=$(printf '%s' "$blob" | awk '/^---$/{f=!f;next} f&&/^status:/{sub(/^status:[[:space:]]*/,"");print;exit}')
# Read the LAST ## Review block's verdict (later reviews override earlier).
verdict=$(printf '%s' "$blob" | awk '
  /^## Review/ { r=1; v=""; next }
  /^## / { if (r) r=0 }
  r && /^verdict:/ { sub(/^verdict:[[:space:]]*/,""); v=$0 }
  END { if (v!="") print v }
')

if [[ "$status" != "review" ]]; then
  echo "refuse merge: task '$slug' status is '$status', must be 'review'." >&2
  echo "the worker must self-validate against ## Acceptance (record ## Validation) and set status: review." >&2
  exit 1
fi
if [[ "$verdict" != "approved" ]]; then
  echo "refuse merge: no approved review verdict on '$slug' (found: '${verdict:-none}')." >&2
  echo "assign a reviewer: scripts/review.sh $slug" >&2
  exit 1
fi

git checkout "$trunk" >/dev/null 2>&1

merge_log=$(mktemp)
if ! git merge --no-ff "$branch" -m "plan: merge $slug" >"$merge_log" 2>&1; then
  conflicted=$(git diff --name-only --diff-filter=U 2>/dev/null || true)
  git merge --abort 2>/dev/null || true
  cat "$merge_log" >&2
  rm -f "$merge_log"
  echo >&2
  echo "merge conflict in: ${conflicted:-<unknown>}" >&2
  echo >&2
  echo "The worker must rebase onto fresh trunk and resolve:" >&2
  echo "  cd $wt" >&2
  echo "  git rebase $trunk   # resolve conflicts, git rebase --continue" >&2
  echo "  # then re-run: scripts/merge-task.sh $slug" >&2
  exit 1
fi
rm -f "$merge_log"

# Flip the task to done on trunk (post-merge the file is in the working tree).
date="$(date +%F)"
sed -i -E \
  -e "s/^status: .*/status: done/" \
  -e "s/^updated: .*/updated: $date/" \
  "$f"
git add "$f"
git commit -q -m "plan: mark $slug done"

git worktree remove "$wt" 2>/dev/null || true
git branch -d "$branch" 2>/dev/null || true

echo "merged $branch into $trunk; $slug done"
