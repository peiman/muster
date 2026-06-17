# muster — SPEC v2 (honest glue: no stale green, safe path is the default)

> *پندارِ نیک، گفتارِ نیک، کردارِ نیک* — good thoughts, good words, good deeds.
> *Skynda långsamt.*

**muster** is a minimalistic, AI-first command-line **ledningssystem** for startups
and midsize companies — agent-drivable, dual-surface (human + `--output json`),
and **glue**: it points at the existing ways a team works and resolves truth from
them, never copying or replacing them.

## v0 + v1 are BUILT — this SPEC specifies the v2 hardening

Do NOT regress v0/v1; `just check` must stay green; every existing command keeps
working. Already built:
- **v0:** the Process spine (recursive graph, cycle detection), Control / Incident
  / Nonconformity / Evidence, hypothesis lifecycle, `revise --because` feedback,
  the enforcement-ladder `check` seam, dual-surface CLI, `readiness`.
- **v1 (the glue engine):** resolvable `Ref` (`file_anchor` + `command` + `note`),
  status/title **derived on read** by resolving the ref, the four honest states
  (**Derived / Stale / Unresolved / Asserted**), `import` (controls as references
  from a TOML/JSON manifest), N:M control↔implementation, dangling-ref detection.
  The honesty rule is enforced at the projection layer (`own_blocks`): a ref-backed
  control cannot show green when its resolved source is Fail/Unresolved/Stale.

A re-dogfood against the real **ckeletin** project confirmed v1 **earns its place**
(5 of 6 v0 findings fixed; resolve-on-read proven; can no longer "lie green").

## Why v2 — the honesty is real but still BOUNDED

The re-dogfood found one load-bearing residual: **muster's honesty is only as
strong as the ref kind the operator happens to pick.**

- A **`command` ref serves a CACHED result for up to a full day** (default
  freshness `MUSTER_FRESHNESS_SECS = 86_400`) without re-running. Reproduced live:
  a command passed, the guarded file was deleted (world went red), and the control
  **still showed green**. That is exactly the `just check`-style honesty gap the v0
  critique centered on — re-opened by the cache.
- **Nothing steers operators to the zero-drift path.** The safe option
  (`file_anchor` pointed at the result artifact the source tool already produces)
  is harder to reach than the drift-prone `--ref-cmd`; the "use sparingly" note
  never appears in CLI help.
- **`file_anchor` is staleness-blind to its source artifact** — it derives a
  true-looking `met` even from a `conformance-report.json` nobody regenerated.
- Anchors are stored unvalidated; a source refactor silently → `Unresolved`.

v2's thesis (Manifesto #9 — Automated Enforcement over advice): **a stale signal
must never show green, regardless of ref kind, and the safe path must be the
default/easy path — enforced structurally, not documented.**

## v2 scope (prioritized from the re-dogfood)

### P0 — Close the command-ref drift window
A `command` ref must **never display green from a result older than its freshness
bound**. Implement EITHER:
- re-resolve command refs **live on read** (like `file_anchor`) by default; or
- keep an opt-in cache but make the **default freshness small**, and a
  past-freshness command result projects to **`Stale`** (which is **not**
  green-eligible — already true in the model; make sure it actually fires).

Regardless of mechanism: **always surface the resolved age** ("served-from-cache,
age=Xs" / a `resolved_age_secs` + `served_from_cache` field) in BOTH human and JSON
output, so a cached verdict is never silently green. A `control resolve [--all]`
command forces a fresh re-resolution.

### P0 — Make the safe path the easy path (steer, don't just warn)
- `control add --ref-cmd …` prints a one-line notice recommending the
  report-artifact path when one is available.
- Add **`--ref-report <path> <anchor>`** sugar = a `file_anchor` pointed at a
  result artifact (e.g. a `conformance-report.json` status), the zero-drift path,
  in one flag.
- `readiness` surfaces each control's **ref-kind drift profile** (live-resolved vs
  cached-command vs stale) so the weakest links are visible.

### P1 — Source-artifact staleness signal for `file_anchor`
Record + surface the **mtime/age of the resolved source file**, so a confidently
`derived: met` value carries an age ("derived from artifact 9 days old"). Optional:
a freshness bound on the *source artifact* that flags it stale in `readiness`.
muster must be honest that its truth is only as fresh as what it points at.

### P1 — Anchor validation + a resolve/doctor surface
- Validate a `file_anchor`'s dotted anchor **resolves at store time**; refuse or
  warn on a dangling anchor at creation, naming the fix.
- `control resolve --all` (or a `muster doctor`) re-resolves every ref and reports
  anchors that have silently gone `Unresolved` after a source refactor.

### P2 — Residual SSOT + propagation + safety
- The decorative on-disk `resolved { value, source_excerpt }` cache for
  `file_anchor` controls is **ignored on read** but is a stored copy of source
  text. Drop it, or mark it explicitly non-authoritative in the schema (SSOT #7).
- **Issue/nonconformity downward propagation:** a failing control flags dependent
  processes for re-evaluation in the feedback model (the partial 6th v0 finding).
- **Cycle-safety test** for readiness traversal across ref-backed controls (prove
  a self/mutual reference can't loop).

## AX / dual-surface conventions (unchanged, non-negotiable)
Every command `--output json` mirrors all fields (no human-only data, no markdown
in JSON); honest exit codes; errors name the fix; `explain` maps intents→commands;
deterministic ordering. New v2 fields (`resolved_age_secs`, `served_from_cache`,
source-artifact age, ref-kind drift profile) appear structurally in JSON.

## Definition of Done — the validator grades this (and we re-dogfood after)

Built on existing v0/v1; **`just check` green; no regression** (v0 + v1 capabilities
still pass). Then, end-to-end, verifiable by a cold agent via `--output json`:

1. **Drift window closed:** create a `--ref-cmd` control whose command checks for a
   file; with the file present it derives green; **delete the file and re-read —
   the control is NO LONGER green** (it re-resolved live, or is `Stale` and
   not-green), and the output shows the resolved age. The exact v0/re-dogfood gap
   is provably shut.
2. **Safe path is reachable + steered:** `--ref-report <path> <anchor>` creates a
   zero-drift report-backed control in one flag; `--ref-cmd` emits the steering
   notice; `readiness` shows the ref-kind drift profile.
3. **Source-artifact age surfaced:** a `file_anchor` control shows the age of its
   source file; a control pointing at an old artifact carries a visible age signal.
4. **Anchor validation:** adding a control with a non-existent anchor is refused or
   flagged at creation; `control resolve --all` flags an anchor that went
   `Unresolved` after the source changed.
5. **No regression:** the v1 re-dogfood chain still passes — import 40 CKSPEC
   controls by reference, derive titles/status from the real
   `/Users/peiman/dev/ckeletin-rust/conformance-mapping.toml` (read-only), N:M
   implementations, honesty rule intact.
6. **(P2 as completed):** decorative cache dropped/marked; cycle-safety test green;
   propagation flag if built.

## Out of scope for v2 (do NOT build)
No embedded LLM. No UI. No network/HTTP ref kind. No re-implementation of
ckeletin's conformance engine — muster POINTS AT it. No multi-tenant/auth. Keep it
minimal: v2 is about *honesty under every ref kind*, not new surface area.

---

## How to build this — the Manifesto (every phase: read and apply it)

Built by autonomous agents (planner, critic, executor, reviewer, validator).
**Every phase reads and develops by Peiman's Manifesto**
(github.com/peiman/manifesto). The validator grades manifesto-alignment — above all
**#9 Automated Enforcement** (the safe path must be *enforced/default*, not merely
documented; a stale signal must be *structurally* unable to show green) and **#7
SSOT / #1 Truth-Seeking** (resolve from the source; surface real freshness; no
stored copy is authoritative). This whole muster arc is **#10** in action: v0 was a
hypothesis a dogfood refuted; v1 fixed it and a re-dogfood confirmed it while
finding this v2 edge. Keep muster honest about its own glue thesis.

The ten principles, verbatim:

1. **Truth-Seeking** — Observe, trace, verify. Every conclusion rests on evidence.
   Assumptions compound into drift; evidence prevents it.
2. **Curiosity Over Certainty** — When something fails, it signals something to
   understand, not merely fix. The failure is the lesson.
3. **Good Will** — Build robustness as the default. The bad must not outgrow the
   good. Build anchors.
4. **Lean Iteration** — Build the smallest thing producing real data. Run it.
   Reality is the specification — not imagination, but observation.
5. **Platforms, Not Features** — Each step is a platform for the next. Build heavy
   enough to support what comes after, clean enough that nothing rots underneath.
6. **Partnership** — Hold each other to high standards. Invest in growth, not just
   output.
7. **Single Source of Truth** — Every piece of information has one authoritative
   location. Reference the source rather than copying it.
8. **Separation of Concerns** — Different responsibilities live in different
   places. Separation enables independent evolution.
9. **Automated Enforcement** — Rules without enforcement erode. Prefer compile-time
   over linting, linting over scripts, scripts over CI, CI over honor systems.
10. **Feedback Cycle** — Specifications and implementations learn from each other.
    A specification is a hypothesis; implementations test it. When better
    approaches emerge, the specification evolves.

> *"The only constant in the fabric of existence is irony — and in the core of
> everything contradictory, beauty lies."* — P.K.
