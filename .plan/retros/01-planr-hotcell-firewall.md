# planr retro — hotcell firewall story

A retrospective from running the `plan` skill (now extracted as `planr`) as a
tech lead across one full story (the `network-firewall` story, 4 tasks, all
merged to trunk) on a real Rust project (`hotcell`). This captures what went
well/poorly within and beyond the scheme's control, plus strengths and
weaknesses to feed back into planr.

Context: tech-lead + worker + reviewer roles; one file per ticket under
`.plan/`; git worktrees as claims; `claim.sh` / `review.sh` / `merge-task.sh`
enforce the flow. 4 tasks shipped; the longest (`loopback-only-net`) needed
two worker attempts (the first was interrupted) and a changes-requested cycle.

## What went well — within control

- **Independent review caught a real failure.** The `loopback-only-net`
  reviewer returned `changes-requested` because `cargo fmt --check` failed
  with 7 diffs that the worker had *claimed* was clean in `## Validation`.
  That's the "no honor system" guard doing real work, not theater. The same
  reviewer independently verified the kernel-enforced deny path (`errno 101`
  on a direct non-loopback connect) rather than trusting the worker's
  self-validation.
- **Pre-seeding the design decision on re-dispatch.** When the first
  `loopback-only-net` worker was interrupted mid-investigation (zero commits),
  I wrote the approach-B conclusion (loopback-only namespace + Unix-socket
  bridge) into the task's `## Notes` and committed it *before* re-dispatching.
  The fresh worker implemented instead of re-deriving the design — a large
  save, and a technique worth formalizing in planr (see "resumption protocol"
  below).
- **Dependency enforcement was automatic.** `claim.sh` refused
  `wire-firewall-into-cli` until `http-connect-proxy` was `done` on trunk; the
  tech lead never had to think about ordering. The `BLOCKED-BY` column on the
  board made the chain legible at a glance.
- **Pre-existing trunk regression triaged cleanly.** A test from the
  `loopback-only-net` task (`run_loopback_network::networked_agent_reaches_loopback_proxy`)
  failed 5/5 on `main` after merge — a partial-`recv` bug the prior reviewer
  had mischaracterized as "transient." I diagnosed it and folded the fix into
  the `firewall-tests` task's scope (it blocked acceptance #3, "suite green").
  Good tech-lead triage, but it surfaced a gap (see below).
- **Honest partial completion.** The `firewall-tests` worker left acceptance #2
  `[ ]` (no real API key available) rather than overclaiming, and the
  reviewer independently agreed the path was proven (real Google 400 through
  the firewall + TLS) while the box wasn't strictly met. The scheme
  encouraged honesty rather than punishing it.

## What went well — without control

- **git worktrees delivered the core invariant.** Zero `.plan/` conflicts
  across 4 tasks with sequential deps — every branch touched only its own
  task file + code, every merge was clean in `.plan/`. The
  bookkeeping-conflict-free property held exactly as designed.
- **Subagent context isolation made the reviewer genuinely independent.**
  Fresh context re-ran the tests; it wasn't influenced by the worker's
  session reasoning.

## What went poorly — within control

- **A broken test shipped to trunk.** Biggest control failure. The
  `loopback-only-net` reviewer approved despite seeing a test failure
  (calling it "transient"); the tech lead (me) merged *without re-running
  the suite on trunk*. "Reviewer approved" was not sufficient — the
  flakiness masked a deterministic failure, and it wasn't caught until
  `firewall-tests` ran the full suite a task later. **The merge is the tech
  lead's gate; approving is not the same as verifying.**
- **`merge-task.sh` doesn't run the project's verification.** It checks
  ticket state (`status: review` + `verdict: approved`) but not build/test
  green. For a code project, that's a real hole — "approved" can still ship
  a red trunk, as it did here. Highest-priority planr fix.
- **Reviewer mischaracterized flakiness.** One reviewer's "transient" was
  deterministic (5/5 fail on trunk). The review process needs network/flaky
  tests run N times, or the merge gate re-runs them. Mitigation belongs in
  planr's reviewer guidance and/or merge gate.
- **`board.sh` `+`-prefix bug recurred.** I'd fixed it earlier, but a skill
  rebuild via `/tmp` copy lost the fix, and I didn't re-validate `board.sh`
  until the in-flight section broke mid-run. Sloppy re-validation on my
  part — but it suggests planr's scripts should have a self-test.
- **`done` drifted from "all acceptance boxes checked."** `firewall-tests`
  merged with #2 unchecked (no real API key). I justified it as "path
  proven, real-key deferred to developer" and noted the waiver in the
  story — reasonable, but it means the board's `done` is no longer a strict
  guarantee. The waiver was ad hoc; planr should formalize it (e.g. a
  `## Waiver` block with reason + owner + follow-up, or a distinct
  `done-with-waiver` status).

## What went poorly — without control

- **No resumption protocol for a dead/interrupted worker.** The first
  `loopback-only-net` attempt ended mid-sentence with zero commits — the
  worktree sat at `in_progress` with no record of how far it got. I had to
  manually inspect `git log` + worktree status to reconstruct state. planr
  has no "resume" path; it assumes workers either finish or commit
  incrementally. (The re-dispatched worker *did* commit incrementally,
  which is why the second attempt was recoverable — but that was the
  worker's discipline, not the scheme's enforcement.)
- **No in-flight visibility.** The `loopback-only-net` worker ran a long
  investigation with no progress signal until it returned. As tech lead I
  was blind during the longest task.
- **Subagent cost doesn't roll up.** I can see my parent-session cost but
  not the workers'/reviewers' — so I can't reason about the true cost of
  this way of working from the tech-lead view. (May be a harness issue
  rather than planr, but worth noting.)

## Strengths

- **File-per-ticket + downward-only links + derived roll-up** genuinely
  produced conflict-free `.plan/` merges. The core data model is sound and
  held up under real concurrency-shaped (sequential here, but the invariant
  is what matters) use.
- **Branch-as-claim + worktrees** eliminates locks and gave clean isolation
  with no checkout juggling. No races, no lockfiles, no manual coordination.
- **Tech-lead-as-single-writer** made slug allocation race-free and kept the
  backlog coherent across the run. Creation never conflicted.
- **Enforceable script gates** (`merge-task.sh` refuses without approved
  verdict; `claim.sh` refuses on unmet deps) made the process reproducible
  rather than conventional — the flow couldn't be skipped by a careless
  agent.
- **The scheme scales to honest partial completion** — worker flagged #2,
  reviewer agreed, story notes recorded the waiver. It didn't force a false
  "done," and it didn't punish honesty.

## Weaknesses / suggested planr improvements

Ordered by priority.

1. **Merge gate must verify, not just trust.** `merge-task.sh` (or the tech
   lead's merge step) should run a project-configurable verification command
   (e.g. `cargo test --release`) and refuse the merge on failure — in
   addition to `status: review` + `verdict: approved`. "Approved" shipped a
   red trunk here. Likely a per-project `verify` hook in the Cellfile/repo
   config that planr calls.
2. **Resumption protocol for interrupted workers.** A `resume.sh <slug>`
   that reads the task's `## Notes`/`## Validation` and reports how far it
   got (last commit, status, uncommitted changes), so a re-dispatched worker
   or tech lead can pick up cleanly. Pair with a worker instruction to
   commit findings incrementally *to the task file* even during
   investigation — not just code — so progress survives an interrupt.
3. **Reviewer guidance for flaky/network tests.** Require running them N
   times (e.g. 3) and recording the repetition in `## Review`, or have the
   merge gate re-run. "Transient" should not be an allowed verdict without
   evidence.
4. **Formalize the done-with-waiver path.** A `## Waiver` block (reason,
   owner, follow-up task) or a distinct status, so `done` stays meaningful
   and waivers are greppable rather than buried in story notes.
5. **Script self-tests.** The `board.sh` `+`-prefix regression recurred
   after a skill rebuild. A tiny test harness for planr's own scripts would
   catch regressions on rebuild/refactor.
6. **In-flight worker progress visibility.** Some heartbeat or incremental
   task-file note the tech lead can read without disturbing the worker.
   Lower priority; the incremental-commit discipline partially covers this.
7. **(Out of planr's scope, noted) subagent cost rollup** so the tech lead
   can see total cost across workers/reviewers.

## Net

The data model and role separation are sound and earned their keep:
conflict-free merges, a real fmt miss caught by independent review, honest
partial completion. The scheme's biggest gap is that **the merge gate trusts
the reviewer instead of the build** — that's how a red trunk shipped. The
second gap is **no resumption protocol** for interrupted workers. Both are
fixable in planr; everything else is polish.
