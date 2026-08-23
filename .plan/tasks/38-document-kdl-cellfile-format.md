---
id: document-kdl-cellfile-format
aliases: [document-kdl-cellfile-format]
kind: task
parent: cellfile-kdl-grammar-and-secrets
title: Document the KDL Cellfile format
status: todo
assignee: null
created: 2026-08-23
updated: 2026-08-23
tags: [docs, cellfile]
depends_on: [replace-cellfile-parser-with-kdl]
---

## Goal

Update the README's Cellfile reference to the KDL grammar, document `env`
passthrough and `secret`, and raise the stated Rust prerequisite.

## Context

The README's "Cellfile format" section is the one reference no `--help`
surface covers, which is why it survived the trim — a user must author a
Cellfile before anything runs. It currently documents the retired
`key = value` format, including `package` and `seed`, which this story
removes.

Follow the `readme` skill: the ownership test, verify every example by running
it, and no status or roadmap content.

## Acceptance

- [ ] The Cellfile reference documents the KDL grammar, including both `env`
  shapes and `secret`, with a worked example that has actually been run.
- [ ] `package` and `seed` are gone from the reference, not marked as
  unimplemented.
- [ ] The endpoint form is `allow "host" port=443`; no `host:port` string.
- [ ] Prerequisites state Rust 1.95+.
- [ ] The secrets behaviour is described where a user acts on it: values come
  from hotcell's environment, are redacted in the plan, are never written to
  cell state, and a missing one is an error.
- [ ] `examples/pi-bootstrap/Cellfile` comments no longer tell the user to
  paste an API key into the file.
- [ ] Every command and Cellfile in the README has been executed as written.

## Notes

- 2026-08-23 created.
