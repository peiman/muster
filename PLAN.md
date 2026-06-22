# PLAN — muster v3: the declarative whole-store round-trip (`state` / `apply`)

> Built on v0/v1/v2. TDD throughout (failing test first → verify red → implement →
> verify green → atomic, conventional commit). `just check` stays green at every
> commit. The customer-grade floor is `bash acceptance/roundtrip.sh` exiting 0
> (committed RED today). Do NOT delete or weaken `acceptance/roundtrip.sh` or
> `acceptance/gate.sh`.

---

## 1. Success Criteria (mechanically verifiable by the Reviewer)

Each criterion is a command the Reviewer can run; the expected result is stated.

**SC-0 — Green gateway, no regression.**
`just check` exits 0 (build + clippy + fmt + nextest workspace + health + coverage
≥ 85%). Every pre-existing test in `crates/**` still passes unchanged. No edits to
v0/v1/v2 *behavior* (only additive: new `state`/`apply` verbs, a new domain
`StoreDocument` type, a refactor of `readiness` into a reusable renderer that
preserves identical output).

**SC-1 — Customer acceptance floor passes.**
`bash acceptance/roundtrip.sh` exits 0 and prints
`PASS: muster v3 declarative round-trip — all 6 acceptance criteria hold.`
This single script mechanically asserts SC-2…SC-7 below (it is the SSOT for them).

**SC-2 — Full read.** `muster state --output json` on a store with ≥1 process and
≥1 ref-backed control emits ONE document whose `data` contains every process,
control, incident, and nonconformity in the store (the editable store, NOT a
`readiness` verdict view). `data.processes`, `data.controls`, `data.incidents`,
`data.nonconformities` are each present (arrays). Read-only: running `state`
twice with no other command leaves `muster state --output json` byte-identical.

**SC-3 — Round-trip is a fixpoint.** Capture `A = state --output json`; wipe the
data dir; `init`; `apply A`; capture `B = state --output json`. `diff A B` is empty.

**SC-4 — Idempotent apply.** Given the captured document `A`, `apply A` then
`apply A` again, then `state --output json`, is byte-identical to `A` (a second
apply changes nothing).

**SC-5 — `--dry-run` mutates nothing but prints the would-be verdict.**
`muster apply --dry-run <changed-but-valid-manifest>` exits 0, prints output
matching `ready|gaps|verdict` (case-insensitive — i.e. a real `readiness` view),
and leaves `state --output json` identical to before the dry-run.

**SC-6 — Fail-closed (all-or-nothing).** `muster apply <manifest-with-a-dangling
file_anchor anchor>` exits non-zero, its error names the offending control id and
the fix, and `state --output json` is byte-identical before and after the failed
apply (the store is left exactly as it was — no partial write).

**SC-7 — Discoverability.** `muster explain` output contains both `state` and
`apply`; `muster catalog --output json` lists subcommands named `state` and
`apply` (catalog is clap-derived, so this is automatic — asserted by a test).

**SC-8 — Honest dual-surface + exit codes.** `state` and `apply` use exit codes
0 (ok) / 1 (command error) only (never the reserved 3). `state --output json` and
`apply --output json` emit the standard CKSPEC envelope (`{status, command, data}`)
on stdout; human mode mirrors the SAME fields with no JSON-only or human-only data.

**SC-9 — Domain purity (#8).** The `StoreDocument` (de)serialization lives in
`crates/domain` and does no I/O and writes no stdout. Disk access stays in
`crates/cli/src/store.rs`; ref resolution stays in `crates/cli/src/resolve.rs`.
The existing architecture-violation tests (domain imports clap/figment/tracing/
infrastructure) remain green.

**SC-10 — Recursive dogfood gate (informational, must not regress).**
`bash acceptance/gate.sh` exits 0 (`muster`'s own `readiness --require-ready`
verdict over a control wired to the live exit code of `roundtrip.sh` is READY).
This passes for free once SC-1 holds; the Reviewer runs it as the closing check.

---

## 2. Architecture

### 2.1 The one schema (#7 SSOT): `domain::StoreDocument`

The manifest IS the store shape. There is no second hand-maintained schema.
Add a pure, serde type in `crates/domain` (a new `crates/domain/src/document.rs`
re-exported from `lib.rs`, or in `store.rs`):

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct StoreDocument {
    #[serde(default)] pub processes: Vec<Process>,
    #[serde(default)] pub controls: Vec<Control>,
    #[serde(default)] pub incidents: Vec<Incident>,
    #[serde(default)] pub nonconformities: Vec<Nonconformity>,
}
```

- Arrays, not maps: every entity already carries its own `id`, and the source
  `Store` is `BTreeMap`-keyed so `.values()` is already id-sorted → deterministic,
  diffable output (#7, AX). `#[serde(default)]` on each field keeps a manifest that
  omits an empty category readable.
- `From<&Store> for StoreDocument` (state direction): collect `s.processes.values()
  .cloned()` etc. — id-sorted, deterministic, NO ref re-resolution, NO mutation.
- `StoreDocument::upsert_into(&self, &mut Store)` (apply direction): for each entity,
  `store.processes.insert(e.id.clone(), e.clone())` etc. — create-or-replace by id,
  no prune. Pure: no validation of refs here (that is the cli's resolution job),
  only the structural merge.

This type is the single source flowing OUT (`state`) and IN (`apply`). Because
`Process`/`Control`/`Incident`/`Nonconformity` are already faithful serde
round-trips (proven by existing tests, e.g. `no_ref_control_serializes_byte_
identical_to_v0`) and `BTreeMap` iteration is id-sorted, `state(apply(state())) ==
state()` holds structurally.

### 2.2 `muster state` — `crates/cli/src/state.rs`

`pub fn execute(output: &Output) -> Result<(), Box<dyn Error>>`:
1. `let s = store::load(&store::data_dir())?;` (errors honestly if uninitialized).
2. `let doc = StoreDocument::from(&s);`
3. Render via `output.success("state", &StateView::new(&doc), &mut io::stdout())`.
   - JSON mode → CKSPEC envelope `{status, command:"state", data:{processes,…}}`.
   - Human mode → a `Display` that lists each category with a count and the entity
     ids/titles (mirrors the same fields; no human-only data, no markdown).
4. NO `store::save` — `state` is structurally read-only.

Reuse the `WithNext` wrapper (human-only "Next:" hint, JSON-transparent) so the
JSON `data` stays a clean serialization of `StoreDocument`.

### 2.3 `muster apply <manifest> [--dry-run]` — `crates/cli/src/apply.rs`

`pub fn execute(args: ApplyArgs, output: &Output) -> Result<(), Box<dyn Error>>`:

1. **Read + parse the manifest (fail-closed on shape).**
   - Read the file (honest error naming the path if unreadable).
   - Parse to `serde_json::Value` (JSON path; `.json`/no-extension). **Envelope
     unwrap:** `state --output json` emits the CKSPEC envelope, so if the parsed
     value is an object containing both `"command"` and `"data"`, take `["data"]`;
     otherwise use the whole value (accept a bare document too). This is what makes
     `apply(state())` work against the literal bytes `state` emitted.
   - `serde_json::from_value::<StoreDocument>(doc_value)` — a malformed shape is
     refused here as a whole, the error naming the parse failure / offending field.
   - (TOML is OUT OF SCOPE for v3 core — JSON is the required, authoritative shape
     the acceptance drives. Do not invent a third schema. A `.toml` manifest may be
     added later by reusing the `toml` crate already in infra; do not build it now.)

2. **Build the merged would-be store in memory (no disk writes yet).**
   - `let mut merged = store::load(&dir)?;` (the current store; `apply` requires an
     initialized store).
   - `doc.upsert_into(&mut merged);` (upsert by id, no prune).

3. **Fail-closed ref validation BEFORE any persist (#9).**
   - For every control in `merged`: validate the control's own `ref` AND each
     implementation's `ref` AND every ref-backed check in every process, by
     resolving it through `resolve::resolve` (the same engine `control add` uses in
     `validate_ref_at_store_time`). Any `file_anchor`/`command` ref that resolves to
     `Resolution::Unresolved` → return `Err` naming the offending **entity id** and
     the fix (e.g. `"refusing to apply: control 'cov-bar' has a file_anchor that
     does not resolve — anchor 'coverage.DOES_NOT_EXIST' not found in '…' …; the
     store was left unchanged"`). A `command` ref's non-zero exit is a legitimate
     *fail*, not an apply error — only an Unresolved (spawn failure / missing file /
     missing anchor / malformed shape) refuses the whole manifest. Generalize the
     existing `control.rs::validate_ref_at_store_time` into a shared cli helper
     (e.g. `resolve::validate_store_refs(&Store) -> Result<(), String>`) so the
     "fix the source" message wording is SSOT.
   - Because validation completes fully before step 4, and step 4 is the only
     writer, a refused manifest leaves the on-disk store byte-for-byte untouched
     (structural all-or-nothing).

4. **Branch on `--dry-run`.**
   - **`--dry-run` (no mutation):** compute `readiness` over `merged` and render the
     would-be verdict via the SHARED readiness renderer (see 2.4). Do NOT call
     `store::save`. Output must contain a real readiness view (matches
     `ready|gaps|verdict`). Return `Ok`.
   - **default (persist):** `store::save(&dir, &merged)?;` then render a success
     summary (counts upserted per category) through the output pipeline.
     - Idempotency (SC-4) is structural: `save` serializes each entity with
       `to_string_pretty`; re-applying the same document writes byte-identical files,
       so a subsequent `state` is byte-identical.

`ApplyArgs` (in `root.rs`): `{ manifest: String, #[arg(long)] dry_run: bool }`.

### 2.4 Reuse `readiness` (SSOT, #7) for `apply --dry-run`

Refactor `crates/cli/src/readiness.rs` so the index-build + `domain::readiness_with`
+ view-render logic is a reusable function operating on an in-memory `Store`, e.g.:

```rust
pub(crate) fn render_for_store(
    s: Store, process: Option<&str>, output: &Output, command: &str,
) -> Result<bool /* is_ready */, Box<dyn Error>>;
```

`readiness::execute` becomes: `load → render_for_store(s, scope, output, "readiness")
→ map is_ready + require_ready to Outcome`. `apply --dry-run` calls
`render_for_store(merged, None, output, "apply")`. The existing readiness JSON/human
output must be unchanged (verified by the existing readiness tests staying green).

### 2.5 Wiring (clap + dispatch)

- `root.rs`: add to `enum Commands`:
  `State` (doc: "Read the entire store as one declarative document (read-only)") and
  `Apply(ApplyArgs)` (doc: "Reconcile the store to a manifest — upsert, idempotent,
  fail-closed"). Catalog inclusion (SC-7) is then automatic (clap-derived).
- `main.rs`:
  - `mod state; mod apply;`
  - `subcommand_name`: add `State => "state"`, `Apply(_) => "apply"` (this `match`
    is exhaustive, so the compiler forces these additions — do not add a fallback).
  - dispatch: `Commands::State => state::execute(&output).map(|()| Outcome::Ok)`,
    `Commands::Apply(a) => apply::execute(a, &output).map(|()| Outcome::Ok)`.
- `explain.rs`: append two `INTENTS` rows so `explain` lists the verbs:
  `("Read the whole store as one document", "muster state --output json")` and
  `("Author/update the whole store from a manifest (the declarative round-trip)",
  "muster apply <manifest>  (preview with --dry-run)")`.
- `lib.rs` (domain): `pub use … StoreDocument;`.

---

## 3. Implementation Checklist (ordered — each step is one atomic, green commit)

> TDD rule for every step: write the failing test FIRST, run it, SEE it fail for the
> right reason, implement the minimum to pass, run `just check`, commit.

1. **`feat(domain): StoreDocument — the one schema in and out`**
   - Test (`crates/domain` `#[cfg(test)]`): `StoreDocument::from(&Store)` collects
     all four categories id-sorted; `upsert_into` creates-or-replaces by id (no
     prune: an entity already present and absent from the doc survives); a
     build→serialize→deserialize→upsert into an empty store reproduces an equal
     `Store` (the in-memory fixpoint).
   - Implement the type, `From<&Store>`, `upsert_into`, derive serde; export from
     `lib.rs`.

2. **`feat(cli): muster state — read the whole store, read-only`**
   - Integration test (`crates/cli/tests/`, new `roundtrip.rs` or extend
     `muster.rs`, `MUSTER_DATA_DIR` temp pattern): seed a process + a ref-backed
     control; `state --output json` envelope `command=="state"` and
     `data.processes`/`data.controls` contain the seeded ids; running `state` twice
     yields identical bytes (read-only); human-mode `state` contains the same ids.
   - Implement `state.rs`, wire `Commands::State` + dispatch + `subcommand_name`.

3. **`refactor(cli): reuse readiness renderer over an in-memory store`**
   - No behavior change: keep all existing `readiness` tests green; add a unit/
     integration check that `readiness` output is unchanged (snapshot a couple of
     known fields). Extract `render_for_store` and re-point `readiness::execute` at it.

4. **`feat(cli): muster apply — upsert, idempotent, fail-closed, dry-run`**
   - Integration tests (red→green), each with a `MUSTER_DATA_DIR` temp store and an
     absolute-path `file_anchor` fixture:
     - **round-trip fixpoint:** capture `state` → wipe dir → `init` → `apply` the
       captured file → `state` equals the capture (SC-3).
     - **idempotent:** `apply` twice → `state` equals the capture (SC-4).
     - **dry-run no-mutation + verdict:** mutate a free-text value in the captured
       doc → `apply --dry-run` exits 0, prints `ready|gaps|verdict`, and `state` is
       unchanged (SC-5).
     - **fail-closed:** break the control's anchor to a dangling segment →
       `apply` exits non-zero, error names the control id, `state` unchanged (SC-6).
     - **envelope unwrap:** `apply` accepts the literal `state --output json`
       envelope bytes (implicitly covered by the fixpoint test).
   - Implement `apply.rs` (parse + envelope-unwrap + merge + validate + dry-run/save),
     the shared `resolve::validate_store_refs` helper, wire `Commands::Apply`,
     dispatch, `subcommand_name`.

5. **`feat(cli): explain + catalog list the two new verbs`**
   - Test: `catalog --output json` `data.commands[].name` includes `state` and
     `apply` (extend the catalog test or add a cli test); `explain` output contains
     both substrings.
   - Add the two `INTENTS` rows. (Catalog needs no code change — clap-derived.)

6. **`test(acceptance): turn the v3 floor green`**
   - Run `bash acceptance/roundtrip.sh` → must print PASS / exit 0 (SC-1).
   - Run `bash acceptance/gate.sh` → must exit 0 / READY (SC-10).
   - Do NOT modify either script. If a script fails, fix the implementation, not the
     script.

7. **`docs(changelog): note muster v3 state/apply round-trip`** (+ README/AGENTS if a
   command table lists verbs). Keep docs honest and minimal; no behavior in this commit.

> Each commit is atomic and `just check`-green. Suggested order keeps the tree green:
> domain type → state → readiness refactor → apply → discoverability → acceptance →
> docs.

---

## 4. Testing Strategy

- **Domain unit tests (pure, fast):** `StoreDocument` from/upsert/serde-round-trip,
  including the no-prune invariant and the empty-category default. These guard the
  one-schema SSOT (#7) and run with no I/O (#8).
- **CLI integration tests (`crates/cli/tests/`, `assert_cmd` + `TempDir` +
  `MUSTER_DATA_DIR`):** mirror the six acceptance criteria as idiomatic Rust tests
  (the TDD red→green lane) so failures are precise and pre-commit-enforced, with
  absolute-path `file_anchor` fixtures for cwd-independent resolution. Cover both
  `--output json` (envelope shape) and human mode (same fields) for SC-8.
- **Black-box acceptance (`acceptance/roundtrip.sh`):** the independent,
  schema-agnostic customer floor (SC-1) — string-replace differs on real `state`
  output, no internal schema knowledge. Driven only by the built binary. The goal of
  the run is to make it exit 0 without weakening it.
- **Recursive dogfood (`acceptance/gate.sh`):** muster's own `readiness
  --require-ready` over a command-ref wired to the live exit code of `roundtrip.sh`
  (SC-10) — the closing confidence check.
- **Regression guard:** full `just check` (workspace nextest + clippy + fmt +
  health + coverage ≥ 85%) at every commit; the architecture-violation tests keep
  domain pure (SC-9); existing readiness tests pin the refactor as behavior-neutral.
- **Edge cases to assert explicitly:** a manifest with a missing/empty category
  (defaults apply); upsert replacing an existing entity by id while leaving an
  unrelated existing entity intact (no prune); a malformed-shape manifest refused as
  a whole; `apply` against an uninitialized store erroring honestly; `--dry-run` over
  a fail-closed (dangling-anchor) manifest still refusing (validation precedes the
  dry-run verdict).

---

## 5. Guardrails

**Must have:** `just check` green at every commit; `acceptance/roundtrip.sh` exit 0;
the one schema (`StoreDocument`) flowing both directions; `state` read-only; `apply`
upsert + idempotent + fail-closed + `--dry-run`; domain-pure (de)serialization;
explain + catalog list both verbs; exit codes 0/1 only for the new verbs.

**Must NOT have:** a second/hand-maintained schema; prune/delete-on-apply; new ref
kinds, network, LLM, UI, merge/3-way semantics, migration tooling, new entity types;
any mutation in `state`; any partial write on a refused `apply`; domain code touching
disk or stdout; deletion or weakening of the acceptance scripts; regression to any
v0/v1/v2 behavior.

---

## 6. Open Questions (resolved by sensible default; flag only if the loop disagrees)

- **Envelope vs bare document for `apply` input:** RESOLVED — `apply` accepts the
  CKSPEC envelope `state --output json` emits (unwrap `.data`) AND a bare document.
  This is required for the acceptance's `apply(state())` to round-trip on the literal
  bytes.
- **TOML manifests:** RESOLVED — out of scope for v3 core (JSON is the required,
  authoritative, acceptance-driven shape). Reuse the existing `toml` crate later if
  needed; do not build it now (#4 minimal).
