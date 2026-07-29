#!/usr/bin/env bash
# Derive a read-only board view from trunk + in-flight branches.
# No checkout required; reads via git show / git branch.
#
# Usage: board.sh [trunk]
# Env:   PLAN_TRUNK (default main), PLAN_DIR (default .plan)
#
# Prints, in order: epics, stories, tasks (from trunk, with BLOCKED-BY for
# tasks whose depends_on are not all done), then an "in flight" section
# scanning plan/* branches for per-task status (so review-ready work shows
# before merge).
set -euo pipefail

trunk="${1:-${PLAN_TRUNK:-main}}"
plan="${PLAN_DIR:-.plan}"

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

# Parse a YAML inline list value like "depends_on: [a, b]" -> one slug per line.
fm_list() {
  awk -v k="$1" '
    /^---$/ { f = !f; next }
    f && $0 ~ "^" k ":" {
      sub("^" k ":[[:space:]]*", "")
      gsub(/^\[|\]$/, "")
      gsub(/[[:space:]]/, "")
      n = split($0, a, ",")
      for (i = 1; i <= n; i++) if (a[i] != "") print a[i]
      exit
    }
  '
}

git rev-parse --verify -q "$trunk" >/dev/null || {
  echo "no such branch: $trunk" >&2
  exit 1
}

# Print the status of a ticket slug on trunk, searching all kinds.
trunk_status() {
  local slug="$1"
  for d in epics stories tasks; do
    local f
    f=$(git ls-tree -r --name-only "$trunk" -- "$plan/$d" 2>/dev/null \
        | grep -E -- "/[0-9]+-${slug}\.md$" | head -n1 || true)
    [[ -n "$f" ]] && { git show "$trunk:$f" | fm_field status; return; }
  done
}

render_section() {
  local label="$1" dir="$2"
  local paths
  paths=$(git ls-tree -r --name-only "$trunk" -- "$dir" 2>/dev/null | grep -E '\.md$' || true)
  [[ -z "$paths" ]] && return
  echo "## $label"
  printf '%-30s %-12s %-22s %-22s %s\n' ID STATUS PARENT BLOCKED-BY TITLE
  while IFS= read -r p; do
    local blob id st pa ti deps blocked d ds
    blob=$(git show "$trunk:$p")
    id=$(printf '%s' "$blob" | fm_field id)
    st=$(printf '%s' "$blob" | fm_field status)
    pa=$(printf '%s' "$blob" | fm_field parent)
    ti=$(printf '%s' "$blob" | fm_field title)
    blocked=""
    if [[ "$label" == "tasks" ]]; then
      deps=$(printf '%s' "$blob" | fm_list depends_on)
      if [[ -n "$deps" ]]; then
        for d in $deps; do
          ds=$(trunk_status "$d")
          [[ "$ds" != "done" ]] && blocked="${blocked} ${d}"
        done
      fi
    fi
    printf '%-30s %-12s %-22s %-22s %s\n' "$id" "$st" "${pa:--}" "${blocked:- -}" "$ti"
  done <<< "$paths"
  echo
}

for entry in "epics:$plan/epics" "stories:$plan/stories" "tasks:$plan/tasks"; do
  label="${entry%%:*}"; dir="${entry#*:}"
  render_section "$label" "$dir"
done

# In flight: scan plan/* branches for the task's status on that branch.
inflight=$(git branch --list 'plan/*' 2>/dev/null | sed 's/^[* ]*//' || true)
if [[ -n "$inflight" ]]; then
  echo "## in flight (worktree branches)"
  printf '%-30s %-14s %s\n' BRANCH STATUS TASK
  while IFS= read -r b; do
    [[ -z "$b" ]] && continue
    slug=${b#plan/}
    f=$(git ls-tree -r --name-only "$b" -- "$plan/tasks" 2>/dev/null \
        | grep -E -- "/[0-9]+-${slug}\.md$" | head -n1 || true)
    if [[ -n "$f" ]]; then
      st=$(git show "$b:$f" | fm_field status)
    else
      st="(no task file)"
    fi
    printf '%-30s %-14s %s\n' "$b" "$st" "$slug"
  done <<< "$inflight"
  echo
fi
