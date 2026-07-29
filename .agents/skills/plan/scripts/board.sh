#!/usr/bin/env bash
# Derive a read-only board view from trunk. No checkout required.
#
# Usage: board.sh [trunk]
# Env:   PLAN_TRUNK (default main), PLAN_DIR (default .plan)
set -euo pipefail

trunk="${1:-${PLAN_TRUNK:-main}}"
plan="${PLAN_DIR:-.plan}"

# Print the value of a frontmatter key from stdin.
fm_field() {
  awk -v k="$1" '
    /^---$/ { f = !f; next }
    f && $0 ~ "^" k ":" {
      sub("^" k ":[[:space:]]*", "")
      print
      exit
    }
  '
}

git rev-parse --verify -q "$trunk" >/dev/null || {
  echo "no such branch: $trunk" >&2
  exit 1
}

for kind in epic story task; do
  case "$kind" in
    epic)  dir="$plan/epics" ;;
    story) dir="$plan/stories" ;;
    task)  dir="$plan/tasks" ;;
  esac
  paths=$(git ls-tree -r --name-only "$trunk" -- "$dir" 2>/dev/null | grep -E '\.md$' || true)
  [[ -z "$paths" ]] && continue
  echo "## $(basename "$dir")"
  printf '%-30s %-12s %-22s %s\n' ID STATUS PARENT TITLE
  while IFS= read -r p; do
    blob=$(git show "$trunk:$p")
    id=$(printf '%s' "$blob" | fm_field id)
    st=$(printf '%s' "$blob" | fm_field status)
    pa=$(printf '%s' "$blob" | fm_field parent)
    ti=$(printf '%s' "$blob" | fm_field title)
    printf '%-30s %-12s %-22s %s\n' "$id" "$st" "${pa:--}" "$ti"
  done <<< "$paths"
  echo
done
