---
id: cli-surface-cleanup
kind: story
parent: post-v1-cellfile-and-provisioning
title: CLI surface cleanup
status: todo
assignee: null
created: 2026-09-05
updated: 2026-09-05
tags: [cli, ergonomics]
---

## Goal

Fix naming and consistency problems in the command-line surface before they
harden into things people script against.

## Scope

- Rename `destroy --force` to `-y`/`--no-confirm`, matching `run` — task
  `rename-destroy-force-to-no-confirm`.

## Notes

- 2026-09-05 created to home a CLI-naming fix with no existing story. The
  parent epic is [[post-v1-cellfile-and-provisioning]] for lack of a CLI/UX
  epic; rehome if one appears.
