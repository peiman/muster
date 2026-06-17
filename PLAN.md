# PLAN — muster v2 (honest glue: no stale green, safe path is the default)

> Manifesto, every step: **#9 Automated Enforcement** (the safe path must be
> *structural/default*, not documented; a stale signal must be *unable* to show
> green), **#7 SSOT** (resolve from the source; no stored copy is authoritative),
> **#1 Truth-Seeking** (surface real freshness as evidence). Build ON v0/v1; do
> NOT regress; `just check` stays green.

This plan is executed verbatim by the Executor. It is grounded in the current
code (file:line references are real as of this iteration). Read the cited
functions before editing them.

---

## 0. Ground truth — where the relevant code lives

| Concern | Location |
|---|---|
| `Ref` enum (`FileAnchor`/`Command`/`Note`), `Outcome`, `value_to_outcome`, `Resolution`, `Derived` (4 states), `is_stale` | `crates/domain/src/reference.rs` |
| Honesty rule `own_blocks` / `effective_status`, `display_title` | `crates/domain/src/model.rs` (`effective_status` ~L514) |
| Cycle detection `detect_cycles`, `successors` | `crates/domain/src/store.rs` (~L631) |
| Resolution policy: `resolve()`, `project()`, `derived_from()`, `is_stale_now()` | `crates/cli/src/resolve.rs` |
| Infra dereference: `resolve_file_anchor()`, `run_command()`, `walk_toml/json` | `crates/infrastructure/src/resolver.rs` |
| Freshness env `MUSTER_FRESHNESS_SECS` (default 86400), `freshness_secs()`, clock `now_iso`, `parse_iso_to_epoch` | `crates/cli/src/store.rs` |
| `control add/show/resolve/add-implementation/import`, `ControlView`, `ImplView`, `resolution_label` | `crates/cli/src/control.rs` |
| CLI flags: `RefFlags`, `ControlSub`, `ReadinessArgs`, `Commands` | `crates/cli/src/root.rs` |
| readiness command (index build + `readiness_with`) | `crates/cli/src/readiness.rs`; domain `crates/domain/src/readiness.rs` |
| Integration tests + helpers (`data()`, `init()`, `write_src()`) | `crates/cli/tests/muster.rs` |
| Gate | `just check` = `ckeletin-check test ckeletin-health` (`Justfile`) |

**Layer rule (compiler-enforced, do not violate):** `crates/domain` is I/O-free
(serde only — no fs/proc). All file/process I/O stays in `crates/infrastructure`.
`crates/cli` is the only bridge. Any new infra dep must be declared in
`ckeletin-project.toml` allowlists with a justification comment.

---

## 1. Success Criteria (each mechanically verifiable by the Reviewer)

All commands below are run with `--output json` against a temp store
(`MUSTER_DATA_DIR`). "Field X present/equals Y" is checked against parsed JSON.
Human (`--output text`) must mirror every field (dual-surface, non-negotiable).

**SC-0 — No regression / gate green.**
`just check` passes (ckeletin-check + full test suite + ckeletin-health). Every
pre-existing v0/v1 test still passes (those that assert the *old* command-cache
default are updated per §3.7, not deleted — the behavior change is intentional
and re-asserted by new tests).

**SC-1 — DRIFT-WINDOW-CLOSED (the headline proof).**
Reproduce the re-dogfood gap and prove it is shut:
1. `touch $TMP/guard.txt`
2. `muster control add C-DRIFT --ref-cmd "test -f $TMP/guard.txt" --ref-dir $TMP`
3. `muster control show C-DRIFT --output json` → `status` is green-eligible
   (`implemented`/met), `resolution.resolution_state == "derived"`,
   `resolution.outcome == "pass"`, and `resolution.resolved_age_secs == 0`,
   `resolution.served_from_cache == false`.
4. `rm $TMP/guard.txt`
5. `muster control show C-DRIFT --output json` **without any intervening
   resolve** → control is **NOT green** (`status != implemented`), and
   `resolution.outcome == "fail"` (live) — and the resolved age is shown.
   The control can never project green from a result older than its freshness
   bound, regardless of ref kind.

**SC-2 — Resolved age always surfaced (human + JSON).**
Every ref-backed control/implementation/check resolution carries
`resolved_age_secs` (integer ≥ 0) and `served_from_cache` (bool) in JSON, and the
human surface prints them (e.g. `derived: pass (Pass) age=0s live`). A cached
verdict is never silently green.

**SC-3 — Safe path reachable + steered.**
- `muster control add C-RPT --ref-report $TMP/conformance-report.json requirements.X.status`
  creates a zero-drift report-backed (`file_anchor`) control in ONE flag; its
  `resolution.resolution_state == "derived"` and ref kind is `file_anchor`.
- `muster control add C-CMD --ref-cmd "..." --ref-dir $TMP` emits a one-line
  steering notice recommending the report path. The notice appears in JSON as a
  structured field (e.g. `steering_notice`) AND in human output.
- `muster readiness --output json` exposes, per ref-backed control, a
  **ref-kind drift profile** field (e.g. `drift_profile`) with values drawn from
  a fixed set: `live_resolved` | `cached_command` | `stale` | `unresolved` |
  `asserted`. The weakest links are visible.

**SC-4 — Source-artifact age for `file_anchor` (P1).**
A `file_anchor` control's resolution carries `source_age_secs` (age of the
pointed-at file by mtime) in JSON + human. A control pointing at an old artifact
shows a visibly large `source_age_secs`; a freshly written one shows ~0.

**SC-5 — Anchor validation at store time + resolve surface (P1).**
- `muster control add ... --ref-file $F --ref-anchor does.not.exist` is **refused**
  with a non-zero exit and an error that names the fix (the path + the missing
  anchor). No control is persisted on refusal (`control list` shows it absent).
- `muster control resolve --all --output json` re-resolves every ref-backed
  control and returns a list flagging any that are `Unresolved` (e.g. after a
  source refactor). Single-id `muster control resolve C` still works.

**SC-6 — No regression of the v1 ckeletin re-dogfood (read-only).**
Against the real, untouched `/Users/peiman/dev/ckeletin-rust`:
- `muster control add CK-ARCH --ref-file /Users/peiman/dev/ckeletin-rust/conformance-mapping.toml --ref-anchor <real CKSPEC anchor>` derives a title/status from the live source; honesty rule intact (cannot show green when source says fail). Read-only: muster never writes into ckeletin-rust.
- The existing v1 import/N:M/honesty dogfood tests/fixtures still pass.

**SC-7 — P2 residuals (as completed).**
- The decorative on-disk `resolved` cache is no longer authoritative: for
  live-resolved refs (`file_anchor`, and `command` in default live mode) muster
  does **not** persist a `resolved` copy that could be read as truth (SSOT #7);
  if retained for opt-in cache mode it is schema-marked non-authoritative.
- A cycle-safety test proves readiness traversal terminates when a ref-backed
  control's graph forms a self/mutual reference (no infinite loop, deterministic
  output).
- (If built) a failing control sets a downward re-evaluation flag on dependent
  processes, surfaced in `readiness`. If not built, it is explicitly listed in
  Open Questions, not silently dropped.

---

## 2. Architecture (the approach)

### 2.1 P0a — Close the command drift window by making *live-on-read the default*
**Decision (Manifesto #9 — structural over documented):** command refs
re-resolve **live on read by default**, exactly like `file_anchor`. There is then
no cache window in which a passed result can outlive reality. The opt-in cache is
retained only behind an explicit switch for genuinely expensive checks; even then,
past-freshness projects to `Stale` (already modeled) and the age is surfaced.

- `crates/cli/src/resolve.rs::project()` — for `Ref::Command`, when cache mode is
  OFF (default), call `resolve(r, now)` live (mirror the `FileAnchor` arm) →
  `served_from_cache = false`, `resolved_age_secs = 0`. When cache mode is ON,
  serve `cached` as today (stale past freshness, age computed).
- Cache mode switch: a new env constant `MUSTER_CMD_CACHE` in
  `crates/cli/src/store.rs` (default `false`/`0`). Default off = honest. Document
  it in code as opt-in for expensive commands only.
- `resolved_age_secs` + `served_from_cache` become **first-class fields on the
  `Derived` projection** (SSOT: computed once, rendered by both surfaces).

### 2.2 Surfacing age: extend `domain::Derived`
Add to `crates/domain/src/reference.rs`:
- `Derived::Derived { …, resolved_age_secs: i64, served_from_cache: bool, source_age_secs: Option<i64> }`
- `Derived::Stale { …, resolved_age_secs: i64, served_from_cache: bool }`
(`Unresolved`/`Asserted` carry none — there is no resolved value.) These are
`#[derive(Serialize)]` already, so JSON gains the fields automatically;
`resolution_label()` in `control.rs` and the readiness renderer print them.
`project()`/`derived_from()` compute the age = `now_epoch − resolved_epoch`
(saturating ≥ 0) and pass the `cached` flag through. Update `outcome()` /
`is_green_eligible()` matches and the in-file unit tests for the new fields.

### 2.3 P0b — Make the safe path the easy path
- **`--ref-report <PATH> <ANCHOR>`**: new flag on `RefFlags`
  (`crates/cli/src/root.rs`) using `num_args = 2`,
  `value_names = ["PATH", "ANCHOR"]`, mutually exclusive with `ref_cmd`/`ref_note`.
  `RefFlags::to_ref()` maps it to `Ref::FileAnchor { path, anchor }` (it is sugar
  for the zero-drift report path). One flag, no `--ref-anchor` pairing needed.
- **Steering notice**: in `control.rs::execute` `ControlSub::Add`, when the built
  ref is `Ref::Command`, attach a structured `steering_notice` string
  ("`--ref-cmd` re-runs a command; prefer `--ref-report <artifact> <anchor>` when
  the tool emits a result file — zero drift, no re-run."). Thread it into the add
  view envelope so it renders in BOTH human and JSON.
- **Ref-kind drift profile in readiness**: classify each ref-backed control into a
  fixed enum (`live_resolved` | `cached_command` | `stale` | `unresolved` |
  `asserted`) derived from its projected `Derived` + ref kind + cache mode.
  Implement classification as a pure helper in
  `crates/domain/src/reference.rs` (e.g. `fn drift_profile(ref, derived, cache_on) -> &'static str`)
  so it is testable + SSOT. Surface per-control in `readiness` JSON and human
  (a compact, deterministically-ordered "drift profile" section).

### 2.4 P1 — Source-artifact age for `file_anchor`
- `crates/infrastructure/src/resolver.rs::resolve_file_anchor()` already opens the
  file; capture `fs::metadata(path).modified()` and return its epoch seconds in
  `FileResolution` (new field `source_mtime_epoch: Option<i64>`). Infra returns
  raw mtime; the cli computes age against `now` (domain stays clock-free).
- `resolve.rs::resolve()` threads the mtime into `Derived::Derived.source_age_secs`
  (computed vs `now`), surfaced in JSON + human for `file_anchor`. Optional
  source-freshness bound (`MUSTER_SOURCE_FRESHNESS_SECS`, default 0 = off) that,
  when set, flags the source stale in the `readiness` drift profile. The required
  deliverable is the *visible age*; the enforced bound is stretch.

### 2.5 P1 — Anchor validation + resolve --all
- **Validate at store time**: in `ControlSub::Add` / `AddImplementation`, after
  building a `file_anchor` ref, call `resolve()` once; if it returns
  `Resolution::Unresolved`, return `Err` naming the fix (path + anchor + reason)
  and do NOT persist. (Command refs: a non-zero exit is a legitimate *fail*, not a
  store-time error — refuse only on a spawn failure / `Unresolved`.)
- **`control resolve --all`**: add `all: bool` to `ControlSub::Resolve` and make
  `id` optional; require exactly one of `--all` or `<id>` (else error naming
  usage). When `--all`, iterate `s.list_controls()`, re-resolve each control + its
  implementations, and emit a report listing every control with its resulting
  `resolution_state`, flagging `unresolved` ones explicitly. Single-id behavior
  unchanged.

### 2.6 P2 — SSOT residual, cycle test, propagation
- **Drop the decorative cache for live refs**: in `control.rs`, stop calling
  `set_control_resolution` / `set_implementation_resolution` for refs that resolve
  live (`file_anchor`, and `command` in default live mode). Persist a cached
  `resolved` ONLY when `MUSTER_CMD_CACHE` is on (the only mode that reads it). If
  the field stays on the struct for that mode, doc-comment it explicitly
  non-authoritative. This removes the stored copy of source text that #7 forbids
  being treated as truth.
- **Cycle-safety test**: add a test building a ref-backed control graph with a
  self/mutual reference and assert `readiness` terminates with deterministic
  output (extends `detect_cycles` coverage in `crates/domain/src/store.rs`).
- **Downward propagation (best-effort, P2)**: if time permits, a failing
  (`outcome == Fail` / `Unresolved`) control sets a re-evaluation flag on
  processes that link it, surfaced in `readiness` as a `flagged_for_reeval` list.
  If not built this iteration, record it in `.omc/plans/open-questions.md` and
  note it in SC-7 — do not silently drop.

### 2.6b Manifesto mapping (the validator grades this)
- **#9** the default command path is live-resolved → drift is *structurally*
  impossible, not merely warned about; `--ref-report` makes the safe path one flag.
- **#7** no authoritative stored copy: cache dropped for live refs; age fields are
  computed from the live source, never trusted from disk.
- **#1** every projection carries `resolved_age_secs` + `source_age_secs` so the
  evidence (freshness) is visible.
- **#10** this whole v2 exists because the v1 re-dogfood refuted "honesty is
  complete" — keep the SPEC↔impl feedback honest.

---

## 3. Implementation Checklist (ordered — Executor follows exactly)

> TDD: for each step write/adjust the failing test first, then implement, then
> `just check`. Atomic, conventional commits (`feat:`/`fix:`/`test:`). Every
> commit must pass `just check`. Domain stays I/O-free.

**3.1 — Age fields on the projection (foundation for everything).**
- `crates/domain/src/reference.rs`: add `resolved_age_secs: i64`,
  `served_from_cache: bool` to `Derived::Derived` and `Derived::Stale`, and
  `source_age_secs: Option<i64>` to `Derived::Derived`. Update
  `outcome()`/`is_green_eligible()` matches; add a pure `drift_profile(...)` helper
  + unit tests; fix the in-file unit tests that construct these variants.
- `crates/cli/src/resolve.rs`: compute age in `derived_from()` (`now_epoch −
  resolved_epoch`, saturating) and pass `served_from_cache = cached`. Update the
  in-file unit tests (they construct `Derived::Derived`/`Stale`).

**3.2 — Command refs resolve live by default (P0a, the drift fix).**
- Add `MUSTER_CMD_CACHE` env + `cmd_cache_enabled()` to `crates/cli/src/store.rs`.
- In `resolve.rs::project()`, branch the `Ref::Command` arm on `cmd_cache_enabled`:
  default → live `resolve(r, now)` (age 0, not cached); cache-on → existing
  cached/stale path. Thread the cache flag through `project`'s callers
  (`ControlView::build`, readiness index, check baking).
- Re-home the resolve.rs unit tests `command_without_cache_is_unresolved` and
  `command_cache_goes_stale_at_freshness_zero` to **cache-mode** behavior; add a
  new default-mode test asserting a command ref re-resolves live (age 0,
  `served_from_cache=false`).

**3.3 — `--ref-report` sugar + steering notice (P0b).**
- `root.rs`: add `ref_report: Option<Vec<String>>` to `RefFlags`
  (`num_args = 2`, `value_names = ["PATH","ANCHOR"]`, conflicts with
  `ref_cmd`/`ref_note`). Map to `Ref::FileAnchor` in `to_ref()`.
- `control.rs` `Add`: if the ref is `Ref::Command`, build a `steering_notice` and
  include it in the add envelope (new optional field on the add view / wrapper) so
  both surfaces render it.

**3.4 — Anchor validation at store time + age surfacing for file_anchor (P1).**
- `infrastructure/src/resolver.rs`: capture source mtime epoch in
  `resolve_file_anchor` → new `FileResolution.source_mtime_epoch`.
- `resolve.rs::resolve()`: thread mtime into `Derived::Derived.source_age_secs`
  (vs `now`); surface in `ControlView`/`ImplView` render + JSON.
- `control.rs` `Add`/`AddImplementation`: for `file_anchor`/`--ref-report`, resolve
  once and refuse (`Err`, no persist) when `Unresolved`, naming the fix.

**3.5 — `control resolve --all` (P1).**
- `root.rs`: `ControlSub::Resolve { id: Option<String>, all: bool }`.
- `control.rs`: when `--all`, iterate all controls, re-resolve, emit a report
  (vector of `{id, resolution_state, unresolved?}`), flag `unresolved`. Validate
  exactly one of `id`/`--all` (else error naming usage).

**3.6 — Readiness drift profile (P0b) + cycle-safety test (P2).**
- `cli/src/readiness.rs`: for each ref-backed control compute `drift_profile`
  (domain helper) from its projected `Derived` + ref kind + cache mode; add to the
  readiness view (JSON + human), deterministic ordering.
- Add the cycle-safety integration test (ref-backed self/mutual reference →
  readiness terminates, deterministic).

**3.7 — P2 SSOT cleanup + regression sweep.**
- Stop persisting `resolved` for live refs in `control.rs` (`Add`,
  `AddImplementation`, `Resolve`); persist only when `MUSTER_CMD_CACHE` on. If the
  struct field stays, doc-comment it non-authoritative.
- Update any v0/v1 test that assumed a persisted `resolved` for live refs to
  assert the live projection instead. Run full `just check`.
- (Optional, time-boxed) downward propagation flag in readiness; else log to
  `.omc/plans/open-questions.md`.

**3.8 — Dogfood + DoD verification (evidence pass, no new behavior).**
- Run the SC-1 drift sequence end-to-end via `--output json`; capture before/after.
- Re-run the v1 ckeletin re-dogfood read-only against
  `/Users/peiman/dev/ckeletin-rust/conformance-mapping.toml` (never write there).
- Confirm `just check` green; confirm dual-surface parity on every new field.

---

## 4. Testing Strategy

**Unit (domain, I/O-free):**
- `reference.rs`: new `Derived` fields in `outcome()`/`is_green_eligible()`;
  `drift_profile()` mapping for each (ref kind × Derived state); `is_stale`
  boundary unchanged.
- `resolve.rs`: `derived_from` age computation (age 0 live; positive when cached;
  saturating on clock skew); command-ref default = live; cache-mode = served/stale.

**Integration (`crates/cli/tests/muster.rs`, via `data()`/`init()`/`write_src()`):**
- **SC-1 drift, the headline test**: create `--ref-cmd "test -f <guard>"`; assert
  green + `resolved_age_secs==0`, `served_from_cache==false`; delete guard;
  re-`show` with no resolve; assert NOT green + `outcome=="fail"` + age shown.
- **SC-2**: assert `resolved_age_secs`/`served_from_cache` present in JSON and in
  the human render for a ref-backed control.
- **SC-3**: `--ref-report PATH ANCHOR` creates a derived `file_anchor` control in
  one flag; `--ref-cmd` add returns `steering_notice` (JSON + human);
  `readiness` JSON carries `drift_profile` per control from the fixed set.
- **SC-4**: write a source file, point a `file_anchor` at it, assert
  `source_age_secs` ~0; (optionally) back-date mtime and assert it grows.
- **SC-5**: add with a bad anchor → non-zero exit, error names path+anchor, no
  control persisted (`control list` absent); `control resolve --all` lists states
  and flags an anchor gone `unresolved` after rewriting the source file.
- **SC-7**: cycle-safety test (self/mutual ref-backed control → readiness
  terminates deterministically); assert default mode does not read a `resolved`
  cache as authority.

**Regression / gate:**
- Full `just check` (ckeletin-check + nextest workspace + ckeletin-health) green.
- Coverage stays ≥ 85% (`just coverage`); add tests for every new branch.
- Existing v0/v1 tests pass; the two resolve.rs cache tests are *re-homed* to
  cache-mode (intentional behavior change, re-asserted), not deleted.
- Dual-surface invariant: for each new field, a test asserts it appears in JSON
  AND is rendered in text (no human-only or json-only data).

**Manifesto-alignment checks (validator grades):**
- #9: a test proving the default command path cannot show green over a deleted
  guard *without* any resolve call (SC-1) — enforcement is structural.
- #7: a test proving live refs do not rely on a persisted `resolved` copy.
- #1: every derived projection exposes `resolved_age_secs` (+ `source_age_secs`
  for file_anchor).

---

## 5. Open questions (mirror to `.omc/plans/open-questions.md`)
- **Validation severity for bad anchors**: refuse (chosen — fail-closed, #9) vs
  warn-and-persist. Plan chooses *refuse*; could add `--allow-dangling` opt-out
  later if it blocks legitimate point-then-create-source workflows.
- **Downward propagation (P2)**: built this iteration only if time remains after
  P0/P1; otherwise deferred and tracked here, not silently dropped.
- **Source-freshness bound** (`MUSTER_SOURCE_FRESHNESS_SECS`): ship the *visible
  age* (required); the optional enforced bound is stretch.

```json
{"rationale": "Plan closes the command-ref drift window structurally by making command refs resolve live-on-read by default (Manifesto #9), surfaces resolved/source age in both human and JSON surfaces, and adds --ref-report sugar, a steering notice, a readiness drift profile, store-time anchor validation, and control resolve --all, with file-grounded ordered steps and DoD-mapped, mechanically verifiable success criteria.", "evidence": {"files": ["PLAN.md"]}}
```
