# PLAN — Harden muster v3 (additive trust-boundary hardening of `state`/`apply`)

> The existing `state`/`apply` implementation is correct and lean. This is **additive
> hardening** — no rewrite. Every change is the smallest mechanism that closes one gap
> (#4 Lean Iteration). `just check` stays green; no regression to v0/v1/v2 or the green
> v3 already on this branch. TDD throughout: failing test first → verify it fails →
> implement → verify it passes → atomic, conventional, green commit.

---

## Context (what exists today, verified by reading the source)

- `domain::StoreDocument` (`crates/domain/src/document.rs`) is the one schema in/out: four
  `Vec` fields (`processes/controls/incidents/nonconformities`), each `#[serde(default)]`.
  `From<&Store>` serializes; `upsert_into(&mut Store)` merges by id (BTreeMap insert → a
  duplicate id in the manifest **silently last-write-wins**).
- `apply` (`crates/cli/src/apply.rs`) today: read file → `serde_json::Value` (malformed-JSON
  error) → **stringly-typed envelope unwrap** (`map.contains_key("command") && contains_key("data")`)
  → `from_value::<StoreDocument>` (wrong-shape error) → `load+upsert_into` → `validate_store_refs`
  (dangling `file_anchor`/`command` ref → refuse) → dry-run branch → `store::save`.
- `store::save` (`crates/cli/src/store.rs`) writes each entity directly with `std::fs::write`
  (one `to_string_pretty(..)+"\n"` per `<id>.json`) — **not atomic**, can tear on ENOSPC.
- `SCHEMA_VERSION` is a **private** `const … = 1` in `crates/cli/src/store.rs`, used only to
  stamp `manifest.json` at `init`. `StoreDocument` has **no** `schema_version`.
- Entity structs (`Process/Control/Incident/Nonconformity`, `crates/domain/src/model.rs`) do
  **not** carry `#[serde(deny_unknown_fields)]` → a misspelled key is silently dropped.
- The interactive mutators (`crates/domain/src/store.rs`) already enforce `validate_slug`,
  `DuplicateId`, and referential existence (`require_process`/`require_control`); `apply` is a
  **back door around them** today (it only validates refs, not id integrity / intra-doc refs).
- `WithNext` (`crates/cli/src/view.rs`) serializes **transparently** — the `state` envelope's
  `data` is a clean `StoreDocument` with no extra keys (confirmed; this is why
  `deny_unknown_fields` on `StoreDocument` is safe — see Architecture §A).
- `Ref`/`Resolution`/`Derived` are **internally tagged** (`#[serde(tag = "…")]`); none of the
  four entity structs use `#[serde(flatten)]` → `deny_unknown_fields` composes cleanly.
- Acceptance: `acceptance/roundtrip.sh` is committed **RED**, now carrying criteria **7
  (duplicate-id refused)** and **8 (unknown-field refused)**. Integration TDD lane:
  `crates/cli/tests/roundtrip.rs`.

---

## Success Criteria (each mechanically verifiable by the Reviewer)

**Gate (the definition of done):**
- **SC-0a** `just check` exits 0 (build + clippy + full test suite + health).
- **SC-0b** `bash acceptance/roundtrip.sh` exits 0 — all 8 criteria, **including the
  committed-RED 7 (duplicate-id refused) and 8 (unknown-field refused)**. `roundtrip.sh` is
  **not** modified or weakened.

**Trust boundary — `apply` validates the full matrix before the single persist, fail-closed:**
- **SC-1 (unknown fields)** `#[serde(deny_unknown_fields)]` is present on `StoreDocument` and on
  `Process`, `Control`, `Incident`, `Nonconformity`. A manifest with a bogus key on any of these
  is **refused** (exit 1), store left byte-identical. The `state→apply` round-trip still holds
  (state emits only known fields). Verified by roundtrip.sh #8 + a new integration test.
- **SC-2 (id integrity)** A new **domain-pure** `StoreDocument::validate(&self, merged: &Store)`
  runs in `apply` **beside** `validate_store_refs`, **before** `store::save`, and refuses:
  - any entity id that is **not a valid slug** (reuses `domain::validate_slug`);
  - a **duplicate id within a category** (scanned over the manifest's `Vec`s, since the merged
    BTreeMap has already deduped). Verified by roundtrip.sh #7 + integration tests.
- **SC-3 (intra-document refs)** `validate` also refuses a `process_ref` / `control_ref` /
  `step.controls` / process-`controls` link that resolves in **neither the manifest nor the
  existing store** (checked against `merged`). Error names the offending entity + the dangling
  id. Verified by an integration test (dangling intra-doc ref → exit 1, store unchanged).
- **SC-4 (offender naming)** With **two** ref-backed controls where only **one** is broken, the
  fail-closed error text contains the broken control's id and **not** the healthy one's.

**All-or-nothing persistence:**
- **SC-5 (atomic save)** `store::save` writes via **temp-then-rename**: every entity is
  serialized to a temp file first (any serialize/IO failure aborts **before any live file is
  touched**), then temps are renamed into place. After a **successful** `apply`, the data dir
  contains **no** stray temp/staging files (`find "$DATA" -name '*.tmp'` is empty), and the
  round-trip stays **byte-identical** (rename does not perturb bytes). Atomicity-on-FAILURE is
  the load-bearing claim, so it is verified directly: a **store-layer test forces a save failure
  after a live entity file exists** (pre-creates `<id>.json.tmp` as a directory so phase-1 staging
  fails) and asserts the pre-existing `<id>.json` is **byte-unchanged** (RED under a direct write,
  GREEN under temp-then-rename). The no-leftover claim is verified by an integration test asserting
  no `*.tmp` remains. (`roundtrip.sh` carries no atomic assertion — its #7 is the duplicate-id check.)

**Versioning:**
- **SC-6 (schema_version, SSOT)** `SCHEMA_VERSION` is the **single source** (moved to
  `domain`, reused by both the manifest stamp and `StoreDocument`). `state` emits a
  `schema_version` field; `apply` of a manifest whose `schema_version` **exceeds** the binary's
  is refused with an honest error (not a silent misparse); an **unversioned** manifest defaults
  to **v1** and is accepted. Verified by integration tests + roundtrip.sh #8 staying green.

**Closed test holes (mutation-detection gaps):**
- **SC-7 (timestamped fixpoint)** `roundtrip.rs` fixpoint **and** `--dry-run` tests seed a
  `command`-ref under `MUSTER_CMD_CACHE=1` resolved to carry a `resolved_ts`, and assert
  byte-identity **including the unchanged timestamp** (catches an apply re-stamp / drop-timestamp
  regression that a timestamp-free seed misses).
- **SC-8 (multi-entity ordering)** A fixpoint test with **≥2 processes and ≥2 controls inserted
  out of id order** proves deterministic ordering survives the disk round-trip.
- **SC-9 (direct negatives)** Integration tests pin: **malformed JSON**, **missing file**, and
  **empty `{}` manifest** behaviors (each with an explicit, asserted outcome).

**Robustness / clarity:**
- **SC-10 (typed envelope unwrap)** The `map.contains_key` probe in `apply.rs` is replaced by a
  typed `#[serde(untagged)]` enum, **keeping the two honest error messages** (malformed-JSON via
  the `Value` parse; wrong-shape via the typed parse). Bare document **and** enveloped document
  both still apply (existing `apply_accepts_a_bare_document_without_the_envelope` test stays green).
- **SC-11 (doc wording)** `apply --dry-run`'s help/doc states it prints the would-be readiness
  verdict but does **not** gate the exit code. (`state`/`apply` use exit 0/1 only — #1
  Truth-Seeking: doc words match the mechanism.)

---

## Architecture

### §A Why `deny_unknown_fields` is safe here (no round-trip regression)
`state`'s JSON `data` is a transparent serialization of `StoreDocument` (WithNext adds nothing to
JSON). Every entity field is either always-serialized or `skip_serializing_if` (absent ⇒ serde
`default` on read). So state never emits a key the structs don't declare ⇒ `apply(state())` never
trips `deny_unknown_fields`. The four structs use no `#[serde(flatten)]`, and `Ref` is internally
tagged (its `kind` discriminator is Ref's concern, not the entity's), so the attribute composes.
Nested structs (`Step`, `Check`, `Implementation`, `Evidence`, `Revision`, `LogEntry`) are **out of
scope** for the required deny set (SPEC names the four + `StoreDocument`); leave them unchanged to
keep the change minimal and the diff reviewable.

### §B `schema_version` as SSOT (#7)
Move the constant to the domain crate so the one schema number flows in and out from one place:
- Add `pub const SCHEMA_VERSION: u32 = 1;` to `crates/domain/src/store.rs`; re-export it as
  `domain::SCHEMA_VERSION` from `crates/domain/src/lib.rs`.
- `crates/cli/src/store.rs`: delete the private `const SCHEMA_VERSION` and `use domain::SCHEMA_VERSION`
  (the `init` manifest stamp keeps writing `{"schema_version": 1}` — byte-identical, no regression).
- `StoreDocument` gains `pub schema_version: u32` with `#[serde(default = "default_schema_version")]`
  where `default_schema_version()` returns `SCHEMA_VERSION` (⇒ unversioned manifest = v1). Declare it
  **first** so state output leads with it deterministically. **Replace `#[derive(Default)]` with a
  hand-written `impl Default`** that sets `schema_version = SCHEMA_VERSION` and empty vecs (a derived
  `Default` would wrongly yield `0`). `From<&Store>` sets `schema_version: SCHEMA_VERSION`.

### §C Domain-pure validation (#8, #9)
New `impl StoreDocument { pub fn validate(&self, merged: &Store) -> Result<(), DomainError> }` in
`document.rs` (pure — no I/O, no stdout). It is the structural sibling of `validate_store_refs`
(which stays in `cli/resolve.rs` because it does live ref I/O):
1. **id integrity** — iterate each manifest `Vec` (`processes/controls/incidents/nonconformities`);
   per category track a seen-set; `validate_slug(id)?` each; on a repeat id return
   `DomainError::DuplicateId { kind, id }`.
2. **intra-document refs against `merged`** (present in manifest ∪ existing store):
   - `process.controls[*]` → `merged.controls`
   - `process.steps[*].process_ref` → `merged.processes`; `process.steps[*].controls[*]` → `merged.controls`
   - `incident.process_ref` → `merged.processes`
   - `nonconformity.process_ref` → `merged.processes`; `nonconformity.control_ref` → `merged.controls`
   On a miss return `DomainError::MissingReference { kind, id, fix }` whose text names the **offending
   entity** and the dangling id (so SC-4's "name the offender, not the healthy one" holds).
   (Note: the document shape has no `from_incident` field — it is a transient CLI arg the interactive
   path resolves into `source`+`process_ref`; the persisted nonconformity carries `process_ref`/
   `control_ref`, which are what we validate.)

`apply` calls `merged… ; doc.validate(&merged)? ; resolve::validate_store_refs(&merged)?` **before**
the `--dry-run` branch, so an invalid manifest is refused in dry-run too, and (because the single
writer `store::save` is reached only after both pass) a refused apply is structurally all-or-nothing.

### §D Atomic `store::save` (temp-then-rename, #3, not a transactional store, #4)
Keep the public signature `pub fn save(dir: &Path, store: &Store) -> Result<(), StoreError>`.
Two phases:
1. **Serialize + stage**: for every entity in all four categories, compute `to_string_pretty+"\n"`
   (byte-identical to today) and write it to a temp file `<sub>/<id>.json.tmp`; collect
   `(temp_path, final_path)` pairs. **Any serialize or write error aborts here**, best-effort removes
   the temps already written, and returns `Err` — **no live `<id>.json` touched yet**.
2. **Commit**: `std::fs::rename(temp, final)` each pair (atomic per file on the same filesystem).
After phase 2 no `*.tmp` remain. `load()` already ignores non-`.json` files, so a temp's `.tmp`
extension is invisible to readers even between phases. (Per #4 this is temp+rename, **not** a
cross-file transaction; the common tear cause — ENOSPC / interruption mid-serialize — is fully
covered because it happens in phase 1 before any live file changes.)

### §E Typed envelope unwrap (#1, SC-10)
Replace the `contains_key` probe with a typed unwrap that keeps the two honest messages:
```rust
#[derive(Deserialize)]
#[serde(untagged)]
enum Manifest {
    Enveloped { data: serde_json::Value }, // ignores status/command/next (NOT deny_unknown_fields)
    Bare(serde_json::Value),               // a bare document
}
```
Flow in `apply`:
1. `serde_json::from_str::<serde_json::Value>(&text)` → **malformed-JSON** error (message unchanged).
2. `serde_json::from_value::<Manifest>(value)` → unwrap to the inner `data` `Value` (Enveloped first
   so an envelope is matched before Bare; a bare doc has no `data` key ⇒ falls through to Bare).
3. `serde_json::from_value::<StoreDocument>(data)` → **wrong-shape / unknown-field** error. Doing the
   `StoreDocument` parse as the final step (rather than embedding it in the untagged variant) keeps
   serde's `deny_unknown_fields` message — which **names the offending field** — intact (an untagged
   enum would swallow it into a vague "did not match any variant"). This satisfies "use a typed
   untagged enum" while preserving #1 Truth-Seeking error quality.

After parse, immediately: `if doc.schema_version > domain::SCHEMA_VERSION { return Err(honest msg) }`.

### §F Doc wording (SC-11)
In `crates/cli/src/root.rs`, `ApplyArgs::dry_run` doc comment: state it prints the would-be readiness
verdict but does **not** gate the exit code (`apply` uses 0/1 only). Mirror the clarification in the
`apply.rs` module doc if it implies otherwise.

---

## Implementation Checklist (ordered; each step is RED → GREEN → atomic commit)

> Run `just check` before starting (must be green). Each step: write/adjust the failing test
> first, run it to see it fail for the right reason, implement, run to green, then
> `git commit` (conventional message). Keep test + implementation in one atomic commit per the
> project's TDD rule.

1. **SSOT `SCHEMA_VERSION` move** (`feat(domain): SCHEMA_VERSION is the one schema number`)
   - Add `pub const SCHEMA_VERSION: u32 = 1;` to `crates/domain/src/store.rs`; export from `lib.rs`.
   - In `crates/cli/src/store.rs` remove the private const and `use domain::SCHEMA_VERSION`.
   - Test: a domain unit test asserts `domain::SCHEMA_VERSION == 1`; existing init/manifest tests
     stay green (manifest still `{"schema_version":1}`).

2. **`StoreDocument.schema_version` + emit on `state`** (`feat: state emits schema_version`)
   - Add the field (declared first), `default_schema_version`, manual `impl Default`, set it in
     `From<&Store>`.
   - Tests: (a) `state --output json` `data.schema_version == 1`; (b) the existing
     `omitted_categories_default_to_empty` parse still works and yields `schema_version == 1`;
     (c) `in_memory_round_trip_is_a_fixpoint` still passes.

3. **`apply` refuses a newer schema_version** (`feat: apply rejects a future schema_version`)
   - After parse, `if doc.schema_version > domain::SCHEMA_VERSION { Err(...) }`.
   - Tests: a manifest with `schema_version: 999` → exit 1, store unchanged, error mentions version;
     an unversioned (`schema_version` omitted) manifest → accepted as v1.

4. **`deny_unknown_fields`** (`feat: apply refuses unknown fields (no silent drop)`)
   - Add `#[serde(deny_unknown_fields)]` to `StoreDocument` and to `Process/Control/Incident/
     Nonconformity`.
   - Tests: bogus key on a control → `apply` exit 1, store unchanged (mirrors roundtrip.sh #8);
     confirm `state→apply` round-trip unaffected (existing fixpoint tests green).

5. **Domain-pure `StoreDocument::validate`** (`feat(domain): validate id integrity + intra-doc refs`)
   - Implement per §C; add domain unit tests for: duplicate id, invalid slug, dangling
     `process_ref`/`control_ref`/`step.controls`/process-`controls` (each → the right `DomainError`),
     and a valid document → `Ok`.

6. **Wire `validate` into `apply`** (`feat: apply validates the full matrix before persist`)
   - Call `doc.validate(&merged)?` beside `validate_store_refs(&merged)?`, before the dry-run branch.
   - Tests: roundtrip.sh #7 (duplicate id) passes; integration tests for duplicate id, invalid slug,
     and dangling intra-doc ref → exit 1 + store byte-identical.

7. **Atomic `store::save`** (`feat: atomic store::save via temp-then-rename`)
   - Refactor per §D, signature unchanged.
   - Tests: a **store-layer save-FAILURE test** asserts a pre-existing `<id>.json` is byte-unchanged
     when phase-1 staging fails (RED under a direct write, GREEN under temp-then-rename — the genuine
     atomicity guard); plus, after a successful `apply`, `find <DATA> -name '*.tmp'` is empty AND the
     round-trip is byte-identical (an integration test). Existing idempotency/fixpoint tests stay green.

8. **Typed untagged envelope unwrap** (`refactor: typed untagged envelope unwrap in apply`)
   - Replace the `contains_key` probe per §E; keep both error messages.
   - Tests: malformed JSON → message-1; wrong-shape → message-2; `apply_accepts_a_bare_document…`
     stays green; an enveloped `state` output still applies.

9. **Doc wording** (`docs: clarify apply --dry-run does not gate the exit code`)
   - Tighten `ApplyArgs::dry_run` (and module doc). Pure-doc; `just check` green.

10. **Close the integration test holes** (`test: timestamped fixpoint, ordering, offender, negatives`)
    - **SC-7**: add a helper that seeds a `command`-ref control (`--ref-cmd true --ref-dir .`) and runs
      `control resolve` under `MUSTER_CMD_CACHE=1` so a `resolved` cache with `resolved_ts` is persisted;
      add a fixpoint test and a `--dry-run` test asserting byte-identity **including** that timestamp.
    - **SC-8**: fixpoint test seeding ≥2 processes + ≥2 controls **out of id order** → state→wipe→apply
      →state byte-identical and id-sorted.
    - **SC-4**: fail-closed test with two ref-backed controls, break only one anchor, assert the error
      contains the broken id and **not** the healthy id.
    - **SC-9**: direct negatives — malformed JSON, missing file, empty `{}` manifest — each asserting the
      explicit chosen outcome (malformed/missing → exit 1 with the honest message; `{}` → accepted as an
      empty upsert that prunes nothing, leaving the store unchanged; pin whichever the implementation
      yields and document it in the test name/comment).

11. **Full verification** (`chore: final green`)
    - `just check` green; `bash acceptance/roundtrip.sh` exits 0. Re-read the diff for convention
      violations (SSOT constants, no inline duplicate strings, domain stays I/O-free).

---

## Testing Strategy

- **Two lanes, same contract.** `crates/cli/tests/roundtrip.rs` is the idiomatic Rust red→green lane
  (drives the public CLI in both surfaces); `acceptance/roundtrip.sh` is the independent customer-grade
  floor the validator runs. Do not weaken `roundtrip.sh`; make criteria 7 & 8 pass by mechanism.
- **Domain unit tests** (`document.rs`, `store.rs`, `model.rs`) cover the pure pieces:
  `validate` matrix, `schema_version` default/From, slug + duplicate detection — fast, no fs.
- **CLI integration tests** cover end-to-end behavior through `assert_cmd`: fail-closed exits +
  byte-identical store after refusal, atomic-save no-temp-leftover, typed-unwrap error messages,
  schema_version gate, and the closed holes (SC-4/7/8/9).
- **Byte-identity is the oracle.** Capture `state --output json` to bytes; after wipe→apply→state (or
  after a refused apply) compare raw bytes. The SC-7 timestamped seed makes a re-stamp/drop-timestamp
  regression actually fail (a timestamp-free seed cannot catch it).
- **Negative-path discipline.** Every fail-closed test asserts BOTH exit-non-zero AND store-unchanged
  (all-or-nothing), and — where applicable — that the error **names the offending entity**.
- **Regression guard.** Run the **whole** workspace suite (`just check`, which runs `cargo nextest`/
  `cargo test --workspace` + clippy + health) after each step; v0/v1/v2 and the green v3 must stay green.
- **Manifesto lens (validator grades this):** #1 doc words match mechanism (SC-11, honest errors);
  #3 atomic write is built, not documented-around (SC-5); #9 fail-closed validated before persist
  (SC-1/2/3); #4 smallest change per gap (temp+rename, not a transaction; deny only the four structs);
  #7 one `SCHEMA_VERSION`, one schema in and out (SC-6).

---

```json
{"rationale": "Wrote PLAN.md: additive, TDD-ordered hardening of muster v3 apply (deny_unknown_fields, domain-pure id/intra-doc validation, atomic temp-then-rename save, schema_version SSOT, typed untagged envelope unwrap) plus the closed test holes, with mechanically-verifiable success criteria gated on `just check` + `acceptance/roundtrip.sh`.", "evidence": {"files": ["PLAN.md"]}}
```
