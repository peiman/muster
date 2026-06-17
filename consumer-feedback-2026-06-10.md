# Consumer feedback — v0.2.21 update wave (2026-06-10)

From: agent:workhorse (relayed by Peiman). Context: ran `just ckeletin-update`
on all three real consumers in one pass — workhorse (0.2.13→0.2.21),
agent-chat (0.2.16→0.2.21), ioguard (0.2.16→0.2.21). All three landed green,
but every consumer tripped the two NEW conformance tests, each needing
hand-tailoring. Three repos of evidence, one design gap.

## Finding 1 — layout-assuming tests panic on non-scaffold consumers (HIGH)

`arch_allowlist.rs` and `violation_drift_guard.rs` hard-code the scaffold
layout (`crates/domain/Cargo.toml`, `crates/infrastructure/...`) and PANIC
with `cannot read ...: No such file or directory` when those crates don't
exist.

- agent-chat layers: chat-core / chat-daemon / chat-ingest / chat-mcp → both tests red.
- ioguard layers: ioguard-core / ioguard-cli / ioguard-ffi → both tests red.

Both repos now carry identical hand-added skip-if-absent guards
(see agent-chat commits 89d4be4 + 21feae8; ioguard local main 14d5087).

Suggested fix: detect the scaffold layout ONCE, centrally — e.g. a shared
`fn scaffold_layout_present() -> bool` (or a declared layout in a
project-owned config) that every layout-assuming test consults. A missing
layer crate in a consumer that never had one is not a violation; failing on
it teaches consumers to patch framework files, which is worse than the gap.

## Finding 2 — framework-owned files that demand consumer edits get clobbered on update (the deeper one)

`arch_allowlist.rs` explicitly instructs: "add it to DOMAIN_ALLOWED_DEPS …
and justify why." Workhorse did exactly that (8 justified domain deps —
serde_json/serde_yaml/indexmap/regex/chrono/thiserror/sha2 on top of serde;
commit c4d25d7 on its harden branch). But `.ckeletin/` is framework-owned —
"Do not edit — replaced on update" — so the NEXT `ckeletin-update` will
clobber every consumer's tailored allowlists AND the skip guards from
Finding 1, re-breaking all three repos' gates on every future update.

Suggested fix: move consumer-specific conformance inputs OUT of framework-
owned test files into project-owned config the tests read (e.g.
`ckeletin-conformance.toml` at repo root: layer crate names + per-layer
dep allowlists + justifications). Framework owns the invariant logic;
consumer owns the facts about itself. That also kills the contradiction
between "do not edit" and "edit this constant."

## Finding 3 — scaffold defaults vs mature consumers (LOW, informational)

The default `DOMAIN_ALLOWED_DEPS = ["serde"]` is right for a fresh scaffold
but guaranteed-red for any real consumer. With Finding 2's config split, the
scaffold ships the strict default and consumers grow their declared list
consciously — same intent, no framework-file edits.

## Praise (so the signal is honest)

- The two-tier update gate (compile→rollback, check→fix-forward-uncommitted)
  worked exactly as designed three times out of three; nobody auto-committed red.
- `CKELETIN_UPDATE_RESULT={"status":...}` machine-readable result line: good.
- The allowlist test CONCEPT is right — workhorse's tailoring found zero
  actual violations, i.e. the architecture held; only the defaults/ownership
  model needs the rework above.

— workhorse, 2026-06-10

---

## ADDENDUM — 0.2.22 validated against all three consumers (same day)

Config-first migration ran on workhorse (0.2.21→0.2.22), agent-chat, ioguard:
`ckeletin-project.toml` written+committed BEFORE each update, per Peiman's
sequencing. Result: **three for three, one green pass each** —
`CKELETIN_UPDATE_RESULT status:updated, committed:true, rolled_back:false`
in every repo. No fix-forward states, no hand-patching of framework files,
the 0.2.21 hand-edits clobbered harmlessly as designed.

Mapping note from the non-scaffold consumers: both declared domain + cli and
left `infrastructure = []` — their adapter crates (chat-daemon, ioguard-ffi)
import the core BY DESIGN, which ckeletin-infrastructure forbids. The empty-
list skip made an honest declaration possible instead of a forced or false
one. Worth a line in the docs: "declare only the layers your architecture
actually has; an empty layer list is an honest answer."

Findings 1–3 from this file: all addressed by 0.2.22. The feedback loop
closed in under a day. — workhorse

---

## FINDING 4 — scaffold-leftover class, instances 3 and 4: repo-specific names baked into scaffolded CI/recipes (HIGH for scaffold consumers)

Context: a workhorse hygiene run on ioguard (public security repo) fixed its
broken pipelines — github.com/peiman/ioguard PR #4, all checks green. Two of
the three defects were the SAME class as this file's Finding 1, in new places:

- **Instance 3 — release workflow binary name.** `.github/workflows/release.yml`
  as scaffolded uploads `target/release/ckeletin-rust`. ioguard's binary is
  `ioguard`, so the v0.1.0 'Publish release' job died with `no matches found`
  — the tag exists but the release NEVER published artifacts, and nobody
  noticed until a CI audit four days later. A release pipeline that fails only
  AT RELEASE TIME is the worst place to discover a scaffold leftover.

- **Instance 4 — SBOM recipe crate path.** The `ckeletin-sbom` recipe
  referenced the scaffold's crate path; on ioguard's layout it exited 1 inside
  CI's SBOM job (fixed via recipe overrides — which work well, good seam).

Class diagnosis: the scaffold ships files containing the scaffold's OWN
identity (binary name, crate paths, repo slug) that every consumer must
remember to hand-edit. The 0.2.22 ckeletin-project.toml move solved this for
conformance facts; the same principle applies to CI/recipe facts.

Suggested fixes, in preference order:
1. **Parameterize at init**: init.sh already knows the project name — have it
   substitute binary/crate names into release.yml and the recipes the way it
   does elsewhere. Leftover becomes impossible.
2. **Or read from one place**: recipes/workflows derive the binary name from
   `cargo metadata` (default-run / first bin target) instead of a literal.
3. **And a cheap guard either way**: a conformance check that greps scaffolded
   CI/recipe files for the literal `ckeletin-rust` in non-upstream repos —
   same shape as the existing upstream-slug self-detection in ckeletin-update.
   That single grep would have caught instances 1–4 at scaffold time.

Evidence trail: ioguard PR #4 (fix commits 461ca5d release path, 75466ca sbom
recipe overrides). Praise where due: the recipe-override seam made the SBOM
fix clean, and `CKELETIN_UPDATE_RESULT` + the two-tier gate continue to be
exactly right in daily use (two more flawless update waves today, 0.2.22 and
0.2.23, three consumers each). — workhorse, 2026-06-10

---

## FINDING 5 — the 0.2.24 scaffold-leftover guard does NOT fire for EXISTING consumers (HIGH — the gate it promises doesn't gate)

Caught dogfooding the 0.2.24 update on ioguard. The relayed expectation was:
"the new guard will intentionally fail `just check` on workhorse and agent-chat
after they update, flagging their pre-fix release.yml copies." In practice it
flagged NOTHING, and ioguard is the proof of why that matters:

- ioguard updated 0.2.23 → 0.2.24, `just check` PASSED GREEN, auto-committed (4f84e37).
- ioguard `.github/workflows/release.yml:64` STILL literally contains
  `target/release/ckeletin-rust` — the exact leftover that made v0.1.0 publish
  zero artifacts (Finding 4, instance 3).

Root cause (verified by reading the shipped code): `scan_for_leftovers()` is
correct and well-tested, but EVERY invocation is hermetic. All 14 call sites:
- `scaffold_leftover_guard.rs` ×8 → `TempDir` fixtures
- `scaffold_scan.rs` inline tests ×4 → `TempDir` fixtures
- `init_smoke.rs` ×1 → a FRESH scaffold copy in a temp dir
There is NO test or recipe that calls `scan_for_leftovers(<real workspace root>)`
as part of `just check`. So the guard proves (a) the scan logic is correct and
(b) a freshly-initialized scaffold is clean — but it never scans an EXISTING
consumer's real tree. A consumer that already had the leftover before 0.2.24
(i.e. every current consumer) sails through green.

Net: `init_smoke` covers t=0 (new scaffolds); the unshippability gate for
already-deployed repos — the population that actually has the bug — is missing.

Suggested fix (small, and the pattern already exists in the same dir):
`arch_allowlist.rs` already scans the REAL tree via `workspace_root()` and fails
`just check` on a violation. Add the twin: a framework-owned test in
`.ckeletin/crate/tests/` that calls
`scaffold_scan::scan_for_leftovers(&workspace_root())` against the real root and
asserts `ScanOutcome::Clean` (Upstream short-circuits, so ckeletin-rust itself
stays green). ~10 lines. THAT is what would have failed ioguard's `just check`
and forced the release.yml fix — which is exactly what the guard was for.

Note: ioguard's actual release.yml leftover is already fixed by the pending
ioguard PR #4 (commit 461ca5d) — so this finding is about the GUARD's coverage
gap, not about fixing ioguard. With the real-tree test added, re-running the
0.2.24-style update on any consumer would correctly red-gate until clean.
— workhorse, 2026-06-10
