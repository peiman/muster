# Spec feedback — ckeletin-rust → ckeletin (2026-05)

> **Status: draft for review.** Not submitted upstream. This captures the
> learnings and the refreshed conformance numbers from the May 2026 hardening
> pass so you can decide how to reconcile them with the spec repo's existing
> `conformance/ckeletin-rust.yaml` (which appears to track *workhorse*, dated
> 2026-04-18, 32/35) and the README's stale "Planned" status for Rust.

## 1. Naming tangle to resolve first

The spec repo has one Rust conformance entry, `conformance/ckeletin-rust.yaml`,
but it is labeled "ckeletin-rust *(workhorse)*" and reports 140 tests / 97%
coverage — those numbers are **workhorse**, not the scaffold repo
(`peiman/ckeletin-rust`, ~80 tests). Meanwhile the spec README still lists the
Rust row as "Planned."

Decision needed: does the spec track **the scaffold** (`ckeletin-rust`) or **the
flagship app** (`workhorse`) as the canonical Rust implementation? They have
diverging conformance stories. Options:
- One entry for the scaffold, a separate entry for workhorse; or
- Keep one "Rust" entry and pick which codebase it reflects.

Everything below is the **scaffold repo's** current state.

## 2. Refreshed conformance — ckeletin-rust scaffold

**Spec v0.4.0 — 35/35 met, 0 deferred, 0 feedback signals.** (Was self-reported
32 met / 3 deferred against a stale v0.3.0 snapshot.)

| Domain | Result |
|--------|--------|
| Architecture (7) | met — compile-time, 7 trybuild violation tests |
| Enforcement (7) | met — `just conform` generator gates ENF-005/006/007 in CI |
| Testing (4) | met — 85% coverage gated in CI |
| Output (5) | met — incl. OUT-004 shadow logging (rendered data, audit on by default) |
| Agent Readiness (5) | met |
| Changelog (7) | met |

Notable corrections made this cycle: ENF-005/006/007 moved deferred → met (the
generator existed but was reported absent); TEST-002 coverage is now CI-gated;
OUT-004 completed; the ARCH-006 "~20 lines" claim corrected (main.rs is 102
lines); ARCH-007 enforcement relabeled compile-time → structural/design.

## 3. Cross-implementation learnings (Principle 10 — feedback for the spec)

These are candidates for the spec's `notes` fields or a research doc:

1. **Keep the worked example in the scaffold.** ckeletin-rust's `just init`
   stripped its only demo command (`ping`), leaving an empty command enum that
   would not compile (issue #1). The fix was to *keep* the renamed demo, which
   is what ckeletin-go's scaffold already does. **Suggested spec note:** a
   conformant scaffold's init/customize step SHOULD leave at least one working
   command so the generated project compiles and runs immediately.

2. **A scaffold must gate its own headline flow.** The init bug shipped because
   the protective `init_smoke` test was `#[ignore]`d and run by nothing. The
   broader lesson: **enforcement (CKSPEC-ENF-001/002) applies to the scaffold's
   own tooling, not just the generated project.** A "green out of the box"
   claim needs an automated check that actually exercises the out-of-the-box
   path.

3. **Reporting rots faster than code.** `CONFORMANCE.md` drifted from the code
   within weeks (claimed a generator was absent when it existed; claimed
   coverage was gated when it wasn't). **Suggested spec direction:** the
   conformance report SHOULD be generated/validated from a machine-readable
   mapping and gated in CI, so prose can't silently diverge — stronger than
   ENF-004's "living audit table" as currently worded.

4. **Toolchain pinning is part of conformance for languages with
   version-sensitive enforcement tests.** Rust's trybuild compile-fail snapshots
   broke on a routine rustc 1.95→1.96 bump (an `E0433` rewording), turning the
   architecture-violation tests red for reasons unrelated to the code — on every
   downstream project too. **Suggested spec note:** where enforcement proof
   depends on exact tool output (Rust trybuild, possibly others), the
   implementation SHOULD pin the toolchain and document a refresh procedure, and
   the conformance report MAY record the pinned version.

5. **`violation_evidence` for tooling-enforced claims is legitimate, but name
   the real mechanism.** Closing ckeletin-rust's ENF-007 feedback signals, we
   used `violation_evidence` for CI-run checks/tests (coverage gate, presence
   checks, output tests) — the spec's sanctioned case — while writing real
   violation tests for the generator's own ENF-005/006 logic. The discipline
   that kept this honest: each evidence string points at the specific CI-gated
   artifact that catches a regression, never a path pasted to mute a warning.
   **Affirms** the ENF-006 v0.4.0 guidance ("try harder before evidence").

## 4. Suggested README change

Move the Rust row off "Planned." If the entry tracks the scaffold:
`| ckeletin-rust | Rust | 35/35 met |`. If it tracks workhorse, use its current
numbers and add a separate scaffold row.
