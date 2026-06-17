# muster — SPEC v1 (the glue engine)

> *پندارِ نیک، گفتارِ نیک، کردارِ نیک* — good thoughts, good words, good deeds.
> *Skynda långsamt.*

**muster** is a minimalistic, AI-first command-line **ledningssystem** for startups
and midsize companies: it runs their management system as a living process map,
makes them ISO-certification-ready without a compliance department, and handles
incidents / command & control — usable by AI agents and humans alike (AX-first).

## v0 is BUILT — this SPEC specifies the v1 evolution

v0 already exists in this repo (do NOT regress it; `just check` must stay green):
the **Process** spine (recursive graph, cycle detection), **Control / Incident /
Nonconformity / Evidence**, the hypothesis lifecycle (`proposed → active →
under_review → retired`, derived `proven`), append-only `revise --because`
feedback, the enforcement-ladder `check` seam, the dual-surface CLI
(`--output json` mirrors all fields), `explain`, and the `readiness` gap meter.
Keep all of it.

## Why v1 — the glue thesis, made true

A dogfood of v0 against the real **ckeletin** project (one spec, CKSPEC, with Go +
Rust implementations) found the load-bearing flaw: **v0 is a *recorder* — it stores
copies of truth that lives elsewhere, and lets an operator hand-set a status the
underlying tool would reject.** It could show a control green while `just check`
is red — violating muster's own honesty principle.

**muster must be GLUE, not a second ledger** (Peiman's principle; Manifesto #7
Single Source of Truth — *"reference the source rather than copying it"*; #8
Separation of Concerns). It must **point at the existing ways a team already works
and not break them** — referencing and *resolving* the authoritative sources, never
copying or replacing them. ckeletin already owns conformance (`just check`,
`conform`, `conformance-mapping.toml`); muster's job is the cross-cutting,
spec-as-hypothesis **glue** that points at those sources and tells one honest
story — value that no single repo has, added without touching any repo's tooling.

So v1's core is one inversion: **stored truth → truth resolved on read from the
pointed-at source.**

## The v1 model shift (the core work)

### `Ref` — a resolvable reference (domain)
A new domain type: a typed pointer to an authoritative source. v1 resolver kinds
(keep minimal — these two cover ckeletin):
- **`file_anchor { path, anchor }`** — read a value from a TOML/JSON file at an
  anchor (e.g. `conformance-mapping.toml#requirements.CKSPEC-ARCH-001.title`, or a
  per-requirement status from a derived `conformance-report.json`). This is the
  PRIMARY glue: point at the *result artifact the source tool already produces*,
  don't re-run it.
- **`command { cmd, dir }`** — run a command in a dir; exit 0 = pass, non-zero =
  fail. For sources with no result artifact. (Use sparingly — prefer reading an
  artifact over re-running an expensive check.)

(`note` = opaque/manual stays available but is always surfaced as *asserted*.
A `url` kind is explicitly out of scope for v1 — no network.)

### Resolution (the dereference engine)
- **domain** defines the pure types only: `Ref`, and a `Resolved` result —
  `Resolved { value, resolved_ts, source_excerpt? }` or `Unresolved { reason }`.
  Domain stays I/O-free (ckeletin enforces domain purity — only `serde`; do NOT
  read files or run commands in `crates/domain`).
- **infrastructure** implements the resolver: dereference a `Ref` (read the file +
  extract the anchor; or run the command) → `Resolved`/`Unresolved`. All file/proc
  I/O lives here (respect the layer boundaries + declare any new dep in
  `ckeletin-project.toml` allowlists with a justification comment).
- **cli** wires resolution into `show`/`readiness` and renders both surfaces.

### Controls and Checks become reference-backed
- `Control` and `Check` gain an **optional `ref: Ref`**.
- **When `ref` is present:** `title`, `status`, and (for checks) `last_result` are
  **DERIVED by resolving the ref on read** — not hand-set. The stored title (if
  any) is a fallback display label, never the authority. Add explicit states:
  **`Unresolved`** (pointer can't be followed — dangling/missing) and **`Stale`**
  (resolved, but the cached resolution is older than a freshness bound or the
  source changed). Cache the last resolution for display; mark it honestly.
- **When `ref` is absent:** the v0 hand-set path still works, but is surfaced as
  **`asserted (unverified)`** — never as proven.
- **The honesty rule (closes the Principle-5 hole):** a reference-backed control
  can NEVER show implemented/green when its resolved source says fail/red. Derived
  status always reflects the source.

Backward-compatible: every v0 command keeps working; `ref` is additive
(skip-serialized when absent, so existing stores load unchanged).

### `readiness` becomes a true truth-meter
- Distinguish **derived** (resolved from source at `<ts>`) from **asserted**
  (hand-set, unverified) — extend the existing proven/asserted split.
- Flag **unresolved / stale / dangling** refs as gap findings (not silent green).
- For evidence, **verify the target exists** (file on disk / anchor present), not
  just that a record is present.
- The honest top-line verdict already never reads green over gaps — extend it to
  count unresolved/stale refs as gaps.

## Prioritized scope

- **P0 — Resolvable `Ref` + dereference engine** (`file_anchor` + `command`), the
  domain/infra/cli split above. The missing organ.
- **P0 — Derived control/check status** from resolving the ref, with
  `Unresolved`/`Stale` states and the asserted escape hatch marked unverified.
- **P0 — Title as a resolved projection** (stored title becomes a fallback).
- **P1 — Reference-import from a source manifest:** one command to ingest many
  controls/checks as *references* (not copies) from a TOML/JSON requirements file,
  so the control set is tied to the source. (Validates against ckeletin's
  `conformance-mapping.toml`.)
- **P1 — Staleness / dangling-reference detection in `readiness`** (verify targets
  exist; record resolution timestamps; surface stale results).
- **P1 — N:M control ↔ implementation:** one requirement satisfied by many
  implementations, each with its own *derived* status (CKSPEC-ARCH-001 met by both
  ckeletin-rust AND ckeletin-go), instead of duplicated or coupled state.
- **P2 — Revision propagation:** a `revise --because <signal>` flags the
  controls/processes that reference the revised source so they can be re-evaluated.

## AX / dual-surface conventions (unchanged, non-negotiable)
Every command accepts `--output json` mirroring all fields (no human-only data, no
markdown in JSON); honest exit codes; not-found/invalid errors name the fix;
`explain` maps intents → commands; deterministic ordering. New derived/resolution
fields appear in JSON structurally (`resolved`, `resolved_ts`, `source`, and the
`asserted|derived|unresolved|stale` distinction).

## Definition of Done — the validator grades this (and we re-dogfood after)

Built on the existing muster code; **`just check` green; every v0 capability still
works (no regression)**. Then, end-to-end and verifiable by a cold agent via
`--output json`:

1. A control with `ref: file_anchor` pointing at a TOML/JSON file derives its
   `title` from that file on read; **edit the source value and muster reflects the
   change** on the next read (not stale-forever).
2. A check with `ref: file_anchor` pointing at a result artifact (or `command`)
   derives `pass`/`fail` from the source; **muster cannot mark it pass when the
   source says fail** — demonstrate the attempt is impossible/ignored (the
   Principle-5 honesty hole is closed).
3. A dangling/missing ref resolves to **`Unresolved`** and shows in `readiness` as
   a gap, never a silent green; a stale resolution shows as **`Stale`**.
4. `readiness --output json` distinguishes **derived** vs **asserted** controls and
   counts unresolved/stale as gaps in the honest verdict.
5. **Reference-import (P1):** import a set of controls as references from a
   TOML/JSON requirements file in one command; the imported controls are tied to
   the source (re-resolvable), not transcribed copies.
6. **N:M (P1):** one requirement linked to two implementation processes, each
   carrying its own derived status.
7. **The ckeletin acceptance (read-only on ckeletin):** point a control at
   `/Users/peiman/dev/ckeletin-rust/conformance-mapping.toml#<a real CKSPEC id>`
   and a check at that repo's derived conformance result (or `just check`); muster
   **derives** the status from the real source and **refuses to show green when the
   source is red** — proving muster now points-at-and-resolves rather than copies.

## Out of scope for v1 (do NOT build)
No embedded LLM. No UI. No network (no `url` ref kind yet). No re-implementation of
ckeletin's conformance engine — muster POINTS AT it, never reproduces it. No
multi-tenant/auth. Keep it minimal: the win is honesty (resolve-don't-copy), not
features.

---

## How to build this — the Manifesto (every phase: read and apply it)

Built by autonomous agents (planner, critic, executor, reviewer, validator).
**Every phase must read and develop by Peiman's Manifesto**
(github.com/peiman/manifesto). The validator grades manifesto-alignment — above
all, **#7 (reference the source, don't copy it)** and **#10 (a spec is a
hypothesis; implementations test it; it evolves)** — not just feature presence.
This very SPEC is v1 *because* v0's hypothesis was refuted by a real dogfood (#2:
failures are signals; #4: reality is the spec). Keep muster honest about its own
glue thesis.

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
