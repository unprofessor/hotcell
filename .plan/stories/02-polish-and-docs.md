---
id: polish-and-docs
kind: story
parent: v1-self-hostable-hotcell
title: Polish and docs
status: todo
assignee: null
created: 2026-07-29
updated: 2026-07-29
tags: []
---

## Goal

Polish and docs

## Context

Parent: `v1-self-hostable-hotcell`. Small items that don't block self-hosting
but should land for a clean v1.

- `src/main.rs` carries `#![allow(dead_code)]` labelled "stub modules";
  `src/session.rs` (`Session`, `SessionStatus`) is defined but unused. Decide
  whether to wire durable session records into the CLI or drop the module.
- No `README.md` at the repo root.

## Notes

- 2026-07-29 created.
