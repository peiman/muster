# muster — SPEC v3 (the declarative round-trip: author and read the whole store in one shot)

> *پندارِ نیک، گفتارِ نیک، کردارِ نیک* — good thoughts, good words, good deeds.
> *Skynda långsamt.*

**muster** is a minimalistic, AI-first command-line **ledningssystem** — agent-drivable,
dual-surface (human + `--output json`), and **glue**: it points at the existing ways a
team works and resolves truth from them, never copying or replacing them.

## v0 + v1 + v2 are BUILT — this SPEC specifies the v3 increment

Do NOT regress v0/v1/v2; `just check` must stay green; every existing command keeps
working. Already built and archived in `SPEC-v0.md` / `SPEC-v1.md` / `SPEC-v2.md`:
- **v0:** the Process spine, Control / Incident / Nonconformity / Evidence, hypothesis
  lifecycle, `revise --because`, the enforcement-ladder `check` seam, `readiness`.
- **v1 (the glue engine):** resolvable `Ref` (`file_anchor` + `command` + `note`),
  status/title derived on read, the four honest states (Derived / Stale / Unresolved /
  Asserted), `control import` (controls as references), N:M control↔implementation.
- **v2 (honesty under every ref kind):** command refs re-resolve live by default,
  `--ref-report` zero-drift sugar, ref-kind drift profile, source-artifact age, anchor
  validation at store time, `control resolve --all`, honor-VERIFIED evidence.

## Why v3 — muster has no whole-store round-trip, and that blocks its agent use

muster's verdict engine is strong, but **an agent cannot author or read the whole store
in one operation.** Today driving muster is N+1 commands (`process add`, then
`control add` ×N, `link-control` ×N, `attach-evidence` ×N), and there is **no single
read of the full store** — only per-entity `list`/`show` and the `readiness` *verdict
view*. `control import` is the closest affordance, but it is controls-only and one-way.

This is the load-bearing AX gap on the path to muster's purpose: being the
deterministic, evidence-anchored verdict layer an **autonomous pipeline** (e.g. a
workhorse loop) drives to answer *"did this run achieve its goal?"*. That pipeline must
be able to (1) **emit** a goal-store from a SPEC's acceptance criteria, and (2)
**re-read** it each iteration — both in one shot, reproducibly. v3 builds exactly that
round-trip and nothing more (Manifesto #4 — the smallest thing that produces real data).

## v3 scope (minimal — two commands and their inverse relationship)

### `muster state` — read the whole store, once
Emit the **entire** store — every process, control, incident, nonconformity — as one
document. `--output json` is the machine surface and is the **authoritative** shape;
human mode renders a faithful summary of the same fields (dual-surface, no human-only
data, no markdown in JSON). Ordering is deterministic (the store is already id-sorted
`BTreeMap`s — #7). `state` is **read-only**: it never mutates the store.

### `muster apply <manifest>` — author/update the whole store, declaratively
Reconcile the store to a manifest in one operation:
- **Upsert semantics:** every entity in the manifest is created-or-replaced by id. v3
  does **not** prune entities absent from the manifest (pruning is a sharper, riskier
  semantic — out of scope; see below). This keeps `apply` additive and the round-trip
  exact.
- **Idempotent:** applying the same manifest twice leaves the store byte-identical
  (#10 — a re-run is not a change).
- **Fail-closed (#9):** a manifest whose ref does not validate (a dangling
  `file_anchor` anchor, a malformed shape) is **refused as a whole** — the store is left
  exactly as it was, and the error names the offending entity and the fix. A partial
  apply must never leave the store half-written.
- **`--dry-run`:** compute and print what the store *would* become (and its resulting
  `readiness` verdict) **without mutating** anything. An agent can preview a goal-store's
  verdict before committing it.
- **Format:** JSON (the exact shape `state --output json` emits) is required; TOML MAY
  be accepted too (consistent with `control import`, inferred from extension). Do not
  invent a *third* schema — the manifest IS the store shape (#7, one schema in and out).

### The defining invariant — round-trip is a fixpoint
`state` and `apply` are inverses over the store:
- `apply(state())` leaves the store unchanged (read it back: identical).
- After `apply M`, `state` emits a document that re-applies to the same store
  (`apply(state(apply(M))) == apply(M)`).
This invariant — not a field-by-field schema in this SPEC — is the contract. It is what
makes muster safe for a machine to drive: what it reads, it can write back exactly.

## AX / dual-surface conventions (unchanged, non-negotiable)
Every command `--output json` mirrors all fields; honest exit codes (0 ok, 1 command
error, 2 clap usage, 3 reserved for `readiness --require-ready` — `state`/`apply` use
0/1 only); errors name the fix; `explain` maps intents→commands and MUST list the two
new verbs; `catalog` MUST include them (it is clap-derived, so this is automatic — add a
test that asserts it). Deterministic ordering everywhere.

## Definition of Done — the validator grades this

Built on existing v0/v1/v2; **`just check` green; no regression**. TDD throughout
(failing test first, every commit atomic and green). Then, end-to-end and verifiable by
a cold agent via `--output json`, **the black-box acceptance script
`acceptance/roundtrip.sh` exits 0** — it drives only the built binary and asserts:

1. **Full read:** `muster state --output json` on a store with ≥1 process and ≥1
   ref-backed control emits a single document containing every entity (not a verdict
   view — the editable store).
2. **Round-trip fixpoint:** capture `state` → wipe the data dir → `apply` the captured
   document → `state` again → the two `state` outputs are **identical**.
3. **Idempotent apply:** applying the same manifest twice yields a byte-identical store.
4. **`--dry-run` mutates nothing:** `apply --dry-run` against a changed manifest leaves
   `state` identical to before, while still printing the would-be `readiness` verdict.
5. **Fail-closed:** `apply` of a manifest with a dangling `file_anchor` anchor exits
   non-zero, names the offending control, and leaves the store **unchanged** (verified by
   comparing `state` before and after).
6. **Discoverability:** `muster explain` and `muster catalog --output json` both list
   `state` and `apply`.

(`acceptance/roundtrip.sh` is committed RED — it fails today because the commands do not
exist. Making it pass is the goal; do not delete or weaken it. The loop's own idiomatic
`crates/cli/tests/*.rs` integration tests are the TDD red→green; the script is the
independent, customer-grade acceptance floor the validator runs.)

## Out of scope for v3 (do NOT build)
No prune/delete-on-apply (upsert only). No new ref kinds, no network, no LLM, no UI, no
merge/3-way semantics, no migration tooling. No new entity types. Keep it minimal: v3 is
**one read, one write, one invariant** — the round-trip that makes muster machine-drivable.

---

## How to build this — the Manifesto (every phase: read and apply it)

Built by autonomous agents (planner, critic, executor, reviewer, validator). **Every
phase reads and develops by Peiman's Manifesto** (github.com/peiman/manifesto). The
validator grades manifesto-alignment — above all:
- **#4 Lean Iteration** — `state` is *serialize the existing `domain::Store`*; `apply` is
  *deserialize + persist*. No new schema, no new surface area. The smallest thing that
  produces the round-trip.
- **#7 SSOT** — the manifest IS the store shape. One schema flows in (`apply`) and out
  (`state`); there is no second, hand-maintained format. The domain `Store` is the
  single source.
- **#8 Separation of Concerns** — the (de)serialization is domain-pure; only the `cli`
  store layer touches disk. Domain never writes stdout.
- **#9 Automated Enforcement** — `apply` is *structurally* all-or-nothing and validates
  refs before persisting; an invalid manifest *cannot* half-write the store. `--dry-run`
  makes preview a first-class, enforced affordance, not advice.
- **#10 Feedback Cycle** — this increment exists because using muster as a pipeline's
  goal-verifier *refuted* the assumption that per-entity commands are enough. The spec
  evolved from that observation; this is #10 in action.

The ten principles, verbatim:

1. **Truth-Seeking** — Observe, trace, verify. Every conclusion rests on evidence.
2. **Curiosity Over Certainty** — When something fails, it signals something to
   understand, not merely fix.
3. **Good Will** — Build robustness as the default. The bad must not outgrow the good.
4. **Lean Iteration** — Build the smallest thing producing real data. Run it. Reality is
   the specification.
5. **Platforms, Not Features** — Each step is a platform for the next. Build heavy enough
   to support what comes after, clean enough that nothing rots underneath.
6. **Partnership** — Hold each other to high standards. Invest in growth, not just output.
7. **Single Source of Truth** — Every piece of information has one authoritative location.
   Reference the source rather than copying it.
8. **Separation of Concerns** — Different responsibilities live in different places.
9. **Automated Enforcement** — Rules without enforcement erode. Prefer compile-time over
   linting, linting over scripts, scripts over CI, CI over honor systems.
10. **Feedback Cycle** — Specifications and implementations learn from each other. A
    specification is a hypothesis; implementations test it.

> *"The only constant in the fabric of existence is irony — and in the core of
> everything contradictory, beauty lies."* — P.K.
