#!/usr/bin/env bash
# Scaffold a new ticket file from a template. Run on trunk (tech lead only).
#
# Usage: new-ticket.sh <kind> <slug> <title> [parent-slug]
# Env:   PLAN_DIR (default .plan)
#
# Writes <plan>/<kind-plural>/<NN>-<slug>.md with frontmatter filled and
# prints the path. The caller then fills the body and commits. The parent slug
# is required for stories and tasks and ignored for epics.
set -euo pipefail

kind="${1:?kind required: epic|story|task}"
slug="${2:?slug required}"
title="${3:?title required}"
parent="${4:-}"

plan="${PLAN_DIR:-.plan}"
case "$kind" in
  epic)  subdir="epics" ;;
  story) subdir="stories" ;;
  task)  subdir="tasks" ;;
  *) echo "unknown kind: $kind (want epic|story|task)" >&2; exit 1 ;;
esac

if [[ "$kind" != "epic" && -z "$parent" ]]; then
  echo "parent slug required for $kind" >&2
  exit 1
fi

dir="$plan/$subdir"
mkdir -p "$dir"
last=$(ls "$dir" 2>/dev/null | grep -oE '^[0-9]+' | sort -n | tail -1 || true)
nn=$(printf '%02d' $((10#${last:-0} + 1)))
path="$dir/${nn}-${slug}.md"

[[ -e "$path" ]] && { echo "already exists: $path" >&2; exit 1; }

here="$(cd "$(dirname "$0")" && pwd)"
template="$here/../templates/${kind}.md"
date="$(date +%F)"

# Escape a string for use in a perl s||| replacement: backslash, $, @, and the
# delimiter | are special and must be backslash-escaped.
repl_escape() { printf '%s' "$1" | perl -pe 's/([\\$@|])/\\$1/g'; }

slug_e=$(repl_escape "$slug")
title_e=$(repl_escape "$title")
parent_e=$(repl_escape "$parent")

# Copy first, then edit only the destination (never the template).
cp "$template" "$path"
perl -i -pe "
  s|__SLUG__|$slug_e|g;
  s|__TITLE__|$title_e|g;
  s|__PARENT__|$parent_e|g;
  s|__DATE__|$date|g;
" "$path"

echo "$path"
