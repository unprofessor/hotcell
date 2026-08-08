---
id: gated-tls-mitm-l7-policy
aliases: [gated-tls-mitm-l7-policy]
kind: story
parent: post-v1-advanced-firewall-policy
title: Gated TLS MITM and L7 policy
status: blocked
assignee: null
created: 2026-08-08
updated: 2026-08-08
tags: [firewall, tls, mitm, gated, phase-5]
depends_on: [hardening-regression-suite]
---

## Goal

Enable method/path/header policy only after an explicit decision to make the
host-side firewall a TLS trust root for the agent's traffic.

## Gate

Keep this story blocked until [[approve-tls-mitm-gate]] records a concrete L7
need, privacy/threat-model approval, key storage and rotation approach,
certificate-pinning compatibility decision, and approval to modify sandbox
trust configuration. This is a trust-model change, not a routine feature.

## Context

Current Hotcell is CONNECT-only and deliberately cannot see encrypted request
content. TLS interception needs a per-cell CA, leaf certificate cache, trust
injection through `src/isolation.rs`, a rustls-based proxy path, and a
non-MITM fast path that preserves current behavior when disabled.

## Notes

- Body capture, secret inspection, and external audit forwarding remain out of
  scope even if this story is later unblocked.
