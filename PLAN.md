# PLAN — muster v1, the GLUE ENGINE

> Built on the v0 spine already in this repo. The one inversion: **stored truth →
> truth resolved on read from the pointed-at source.** Manifesto-graded — above all
> **#7 (reference the source, don't copy it)** and **#10 (the spec is a hypothesis;
> v0's "recorder" hypothesis was refuted by the ckeletin dogfood, so v1 evolves it).**
> Additive and backward-compatible: every v0 command keeps working, `ref` is
> skip-serialized when absent, and `just check` must stay green at every step.

---

## 0. Hard architectural constraints (read first — violating any one fails `just check`)

These are compiler-/test-enforced today. The plan is designed around them; do **not**
work around them.

- **C1 — domain is I/O-free.** `crates/domain` may depend only on `serde` + `thiserror`
  (enforced by `ckeletin-project.toml` `[allowlists]` + `arch_allowlist.rs`). NO `fs`,
  NO `std::process`, NO reading files or running commands in domain. Domain defines
  the pure `Ref`/`Resolution` types and the pure projection logic only.
- **C2 — infrastructure must NOT depend on domain or cli** (CKSPEC-ARCH-005, enforced by
  `infra_imports_domain` trybuild test + the allowlist). Therefore the resolver in
  `crates/infrastructure` **cannot return `domain::Resolution`**. It returns
  infrastructure-local plain structs; the **cli** layer bridges domain ↔ infra ↔ domain.
- **C3 — infrastructure must not write to stdout/stderr** (`print_stdout`/`print_stderr`
  = deny). The resolver returns data; cli renders.
- **C4 — every new infra dependency must be added to BOTH `crates/infrastructure/Cargo.toml`
  `[dependencies]` AND `ckeletin-project.toml` `[allowlists] infrastructure = [...]`,
  exactly (the allowlist test asserts an exact set), each with a justification comment.**
- **C5 — dual surface is non-negotiable:** every command supports `--output json`
  mirroring all fields (no human-only strings in `data`, no markdown in JSON); honest
  exit codes; deterministic (id-sorted) ordering. New fields appear structurally in JSON.
- **C6 — domain stays clock-free.** Timestamps (`now`) are computed at the cli boundary
  (`store::now_iso`) and passed *into* domain ops, exactly as v0 does.

The bridge (the key design move forced by C1+C2):

```
cli  reads domain::Ref  ──extract primitives──▶  infrastructure::resolver
                                                  (fs/process I/O, infra-local result)
cli  maps infra result  ──▶ domain::Resolution ──▶ domain projection (pure, honest)
cli  renders both surfaces
```

---

## 1. Success Criteria (mechanically verifiable by the Reviewer)

Each criterion is a command or check the Reviewer can run. SC-0..SC-2 gate everything;
SC-3..SC-13 map to the SPEC Definition of Done items 1–7.

- **SC-0 — No regression / green gate.** `just check` exits 0 (runs `ckeletin-check`,
  the full `cargo test --workspace`, and `ckeletin-health`). Every pre-existing test in
  `crates/cli/tests/muster.rs`, `crates/cli/tests/cli.rs`, `crates/domain/**`,
  `crates/infrastructure/**` still passes unmodified in intent (existing assertions are
  not weakened; new behavior is added, not substituted).
- **SC-1 — Layer purity preserved.** `crates/domain/Cargo.toml` `[dependencies]` is still
  exactly `{serde, thiserror}`; `crates/infrastructure/Cargo.toml` deps match
  `ckeletin-project.toml` `[allowlists] infrastructure` exactly; `arch_allowlist.rs`,
  `infra_imports_domain`, and all violation trybuild tests pass. `grep -rn
  "std::fs\|std::process" crates/domain/src` returns nothing new.
- **SC-2 — `ref` is additive / backward-compatible.** A v0 store written before this
  change (a `control`/`check` JSON with no `ref` field) loads unchanged; serializing a
  control/check with no ref produces JSON byte-identical to v0 (proven by an unchanged
  golden assertion in `muster.rs` + a `skip_serializing_if = "Option::is_none"` /
  `Vec::is_empty` round-trip test in domain).

- **SC-3 (DoD #1) — title is a resolved projection.** A control with
  `ref: file_anchor{path, anchor}` pointing at a TOML or JSON file renders its `title`
  from the file on read. Concretely (integration test): create a temp `src.toml` with
  `[requirements.r1]\ntitle = "Alpha"`; `muster control add c1 --title placeholder
  --ref-file src.toml --ref-anchor requirements.r1.title`; `muster control show c1
  --output json` ⇒ `data.title == "Alpha"` and `data.resolution.resolution_state ==
  "derived"`. Edit the file to `title = "Beta"`; re-run show ⇒ `data.title == "Beta"`
  (NOT stale-forever). The stored `title` ("placeholder") appears only as
  `data.fallback_title`.
- **SC-4 (DoD #2) — checks derive pass/fail from source.** A check with
  `ref: file_anchor` at a status leaf (`status = "met"`) or `ref: command` derives
  `last_result`: `met/pass/0-exit ⇒ pass`, `unmet/fail/non-zero ⇒ fail`. Integration:
  `process check add` with `--ref-file`/`--ref-anchor` or `--ref-cmd`/`--ref-dir`;
  `process show --output json` ⇒ the check's `last_result` equals the derived outcome.
- **SC-5 (DoD #2 — the honesty rule / Principle-5 hole closed).** A ref-backed check
  **cannot be hand-set to pass when the source says fail.** Point a check at a source
  whose value is `unmet`; attempt `muster process check <pid> <check-id> --pass` ⇒ the
  command **errors** with a message naming the ref as the authority (exit ≠ 0). A domain
  unit test asserts: for a ref-backed check, `effective_result()` is the derived outcome
  regardless of any stored `last_result`.
- **SC-6 (DoD #3) — dangling ref ⇒ `Unresolved`, never silent green.** A control/check
  whose `ref` points at a missing file or a missing anchor resolves to
  `resolution_state == "unresolved"` with a `reason` that names the missing path/anchor.
  In `readiness --output json` it appears as a `gap_finding` (kind `ref_unresolved`), and
  the verdict is `GAPS:n`, never `READY`.
- **SC-7 (DoD #3) — stale resolution ⇒ `Stale`.** With `MUSTER_FRESHNESS_SECS=0`, a
  ref-backed entity whose cached resolution is older than `now` surfaces
  `resolution_state == "stale"` (it shows the last value honestly, marked stale) and is
  counted as a gap in `readiness`. (file_anchor refs re-resolve live each read and are
  fresh; `command` refs serve the cache and go stale past the freshness bound — see §2.4.)
- **SC-8 (DoD #4) — readiness distinguishes derived vs asserted and counts ref gaps.**
  `readiness --output json` `data` separates **derived** controls (ref-backed, resolved)
  from **asserted** controls (no ref, hand-set, surfaced as `asserted (unverified)`);
  `gap_findings` includes any `ref_unresolved` / `ref_stale` / derived-`fail` controls;
  the honest verdict counts them (never `READY` while any exists).
- **SC-9 (DoD #5) — reference-import.** `muster control import <manifest> [--format
  toml|json]` ingests every requirement in the manifest as a **reference** (each new
  control carries `ref: file_anchor{path:<manifest>, anchor:<prefix>.<ID>.title}`), not a
  transcribed copy. Integration: import a 3-requirement temp TOML ⇒ 3 controls created,
  each `control show --output json` has a `ref` whose `path` equals the manifest and
  re-resolves; editing a title in the manifest changes the imported control's shown title.
- **SC-10 (DoD #6) — N:M control ↔ implementation.** A single control can carry
  `implementations: [{id, ref}, ...]`; each implementation derives its **own** status.
  Integration: one control with two implementations pointing at two different sources
  (one `met`, one `unmet`) ⇒ `control show --output json` lists both with per-impl
  `resolution`/outcome, and the control's aggregate honest status is **not green** (any
  non-pass implementation blocks green).
- **SC-11 (DoD #7) — the ckeletin acceptance (read-only).** Point a control's ref at
  `/Users/peiman/dev/ckeletin-rust/conformance-mapping.toml` anchor
  `requirements.CKSPEC-ARCH-001.title`, and a check at
  `requirements.CKSPEC-ARCH-001.status` (or `--ref-cmd "just check" --ref-dir
  /Users/peiman/dev/ckeletin-rust`). `muster control show`/`process show --output json`
  **derive** title/status from the real file; muster **refuses to show green when the
  source says red** (test: copy the manifest to a tempdir, flip the status to `unmet` ⇒
  derived `fail` ⇒ control not green). muster reads ckeletin **read-only** (the test never
  invokes a muster mutator against that repo).
- **SC-12 — Manifesto alignment is real, not decorative.** `#7`: muster stores a pointer
  (`Ref`), never a copy of the source's title/status — proven by SC-3/SC-9 (editing the
  source changes muster's output with no muster mutation). `#8`: the domain/infra/cli
  split in §2 holds (SC-1). `#10`: module docs cite the principle numbers they serve, and
  any v0 statement now false is corrected (not duplicated).
- **SC-13 — `explain` and `catalog` updated.** `muster explain` maps the new intents
  ("point a control at a source", "import requirements", "link an implementation") to the
  new commands; `muster catalog --output json` lists every new subcommand/flag. No intent
  references a command that does not exist.

---

## 2. Architecture

### 2.1 New domain module: `crates/domain/src/reference.rs` (pure)

Add `pub mod reference;` to `lib.rs` and re-export the public types.

```rust
use serde::{Deserialize, Serialize};

/// A typed pointer to an authoritative source (#7 — reference, don't copy).
/// v1 resolver kinds only (no `url`/network — out of scope).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Ref {
    /// Read a scalar at a dotted anchor in a TOML or JSON file. The PRIMARY glue.
    FileAnchor { path: String, anchor: String },
    /// Run a command in a dir; exit 0 = pass, non-zero = fail. Use sparingly.
    Command { cmd: String, dir: String },
    /// Opaque/manual — always surfaced as *asserted*, never proven.
    Note { text: String },
}

/// The honest outcome a resolved value implies. Pure mapping (#1 evidence).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome { Pass, Fail, Unknown }

/// Map a resolved scalar to an outcome. A title string ("Four-layer architecture")
/// ⇒ Unknown (title-only, no honesty claim); a status token ⇒ Pass/Fail.
pub fn value_to_outcome(value: &str) -> Outcome {
    match value.trim().to_ascii_lowercase().as_str() {
        "met" | "pass" | "passed" | "ok" | "true" | "green" | "0" => Outcome::Pass,
        "unmet" | "not_met" | "fail" | "failed" | "false" | "red" => Outcome::Fail,
        _ => Outcome::Unknown,
    }
}

/// The result of dereferencing a Ref — pure data. Built by the cli from the infra
/// resolver's output; consumed by the projection below. Cached on the entity (for
/// `command` refs / display) and re-derived for `file_anchor`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Resolution {
    Resolved {
        value: String,
        outcome: Outcome,
        resolved_ts: String,                 // RFC-3339, the display SSOT
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source_excerpt: Option<String>,
    },
    Unresolved { reason: String },
}

/// Pure staleness rule (#1). Epoch seconds in (cli owns the clock, C6); domain just
/// compares. `freshness_secs == 0` ⇒ any cached resolution is immediately stale on the
/// next read (deterministic test hook, SC-7).
pub fn is_stale(resolved_epoch: i64, now_epoch: i64, freshness_secs: i64) -> bool {
    now_epoch.saturating_sub(resolved_epoch) > freshness_secs
}

/// The four honest display states surfaced in JSON (`resolution_state`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "resolution_state", rename_all = "snake_case")]
pub enum Derived {
    Derived  { value: String, outcome: Outcome, resolved_ts: String,
               #[serde(skip_serializing_if = "Option::is_none")] source_excerpt: Option<String> },
    Stale    { value: String, outcome: Outcome, resolved_ts: String },
    Unresolved { reason: String },
    Asserted, // no ref → hand-set, surfaced as "asserted (unverified)"
}
```

### 2.2 Reference-backed Control / Check (additive fields on `model.rs`)

```rust
// On Control:
#[serde(default, skip_serializing_if = "Option::is_none")]
pub r#ref: Option<Ref>,
#[serde(default, skip_serializing_if = "Option::is_none")]
pub resolved: Option<Resolution>,           // cache of the last resolution
#[serde(default, skip_serializing_if = "Vec::is_empty")]
pub implementations: Vec<Implementation>,   // N:M (P1)

// On Check: the same `ref` + `resolved` (no implementations on checks).

/// One implementation of a control's requirement, with its own derived status (P1 N:M).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Implementation {
    pub id: String,
    pub r#ref: Ref,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved: Option<Resolution>,
}
```

Pure projection methods (domain, fully unit-testable; cli supplies the freshly-attempted
`Resolution` + `now_epoch` + `freshness_secs`):

- `Control::project(now_epoch, freshness_secs) -> Derived` — uses `self.resolved` (and for
  N:M, aggregates `implementations`). Honest aggregate: green-eligible only if the
  control's own ref (if any) is `Derived` with `outcome != Fail` AND **every**
  implementation projects to `Derived`+`Pass`. Any `Fail`/`Unresolved`/`Stale` ⇒ not green.
- `Control::display_title() -> &str` — resolved value when `resolved` is `Resolved`, else
  the stored `title` (fallback). Stored title is surfaced as `fallback_title`.
- `Control::effective_status() -> ControlStatus` — when a ref/implementations are present,
  derived (Pass ⇒ may be `Implemented`; Fail/Unresolved/Stale ⇒ forced **not**
  `Implemented`). When no ref ⇒ the stored hand-set status (asserted).
- `Check::effective_result() -> CheckResult` — ref-backed ⇒ the derived outcome
  (`Pass→Pass`, `Fail→Fail`, `Unknown→Unknown`), **ignoring** any stored `last_result`
  (closes the honesty hole, SC-5). No ref ⇒ stored `last_result` (v0 path).

**Honesty rule, concretely (SC-5/SC-11):** a ref-backed control/check derives its
status/result *on read*; the stored value is never the authority. A control can therefore
never display `implemented`/green while its resolved source (or any linked check / any
implementation) is `fail`/`unresolved`/`stale`.

### 2.3 New infra module: `crates/infrastructure/src/resolver.rs` (I/O, NO domain dep — C2)

Infra-local result structs (plain; they never cross the JSON surface — the cli maps them
to `domain::Resolution`):

```rust
pub struct FileResolution {
    pub value: Option<String>,   // None ⇒ anchor/file missing
    pub reason: Option<String>,  // why unresolved (names path/anchor)
    pub excerpt: Option<String>,
}
pub struct CommandResolution {
    pub exit_code: Option<i32>,  // None ⇒ spawn failed
    pub stdout_tail: String,
    pub reason: Option<String>,
}

/// Read `path` (TOML if .toml, JSON if .json — by extension), walk the dotted `anchor`,
/// return the scalar as a string. All fs errors → reason (never panic).
pub fn resolve_file_anchor(path: &str, anchor: &str) -> FileResolution { ... }

/// Run `cmd` in `dir`; capture exit + stdout tail. Spawn failure → reason.
pub fn run_command(cmd: &str, dir: &str) -> CommandResolution { ... }
```

Implementation notes:
- TOML via the `toml` crate, JSON via `serde_json` — both parsed to a generic `Value`,
  then walk the dot-path (`requirements.CKSPEC-ARCH-001.title`). A scalar leaf
  (string/int/bool) → its string form; a non-scalar/missing leaf → `value=None` + a
  `reason` naming the missing segment. **Read-only** (open for read; never write).
- `run_command`: `std::process::Command` (`sh -c <cmd>` in `dir`); exit 0 = pass, non-zero
  = fail, spawn failure = unresolved. Bound captured stdout to a tail (e.g. last 512 bytes).
- **Dependencies to add (C4):** `crates/infrastructure/Cargo.toml` `[dependencies]` gains
  `serde_json = { workspace = true }` and `toml = { workspace = true }`; add
  `toml = "0.8"` to root `Cargo.toml` `[workspace.dependencies]`; update
  `ckeletin-project.toml` to `infrastructure = ["ckeletin", "serde_json", "toml"]` with a
  justification comment per dep (resolver reads JSON/TOML result artifacts — disk boundary, #8).

### 2.4 CLI bridge + new commands (`crates/cli`)

The cli is the only place domain and infra meet. Add `cli/src/resolve.rs`:

```rust
// Given a domain::Ref, call the right infra resolver and build a domain::Resolution.
fn resolve(r: &domain::Ref, now_iso: &str) -> domain::Resolution { ... }
// file_anchor → resolve_file_anchor; map value→Outcome via domain::value_to_outcome.
// command     → run_command; exit 0 ⇒ Pass, else Fail; spawn-fail ⇒ Unresolved.
// note        → Resolved{value:text, outcome:Unknown} (asserted-only at projection).
```

Read-path policy (drives SC-3/SC-7):
- **`file_anchor`** refs are re-resolved **live on every read** (`show`, `readiness`), the
  cache is refreshed, `resolved_ts = now` ⇒ always fresh (SC-3: edits reflected).
- **`command`** refs are **not** auto-run on plain reads; they serve `self.resolved` (the
  cache) and are refreshed only by an explicit `muster control resolve <id>` /
  `muster process check <pid> <cid> --resolve`. A served cache older than `freshness_secs`
  projects to `Stale` (SC-7).
- Freshness bound: `MUSTER_FRESHNESS_SECS` env (default `86400`); cli parses it and the
  cached `resolved_ts` to epoch seconds (add `parse_iso_to_epoch` next to `now_iso` in
  `cli/store.rs`, the inverse of `now_iso`) and calls `domain::is_stale`.

New / extended command surface (clap in `root.rs`; handlers in `control.rs`, the check
handler, plus a new `import.rs`):
- `control add <id> --title <t> [--ref-file <path> --ref-anchor <anchor>] [--ref-cmd <cmd>
  --ref-dir <dir>] [--ref-note <text>] [--clause-ref ..] [--applicable ..]` — at most one
  ref kind; conflicting ref flags error (clap `conflicts_with`).
- `control resolve <id>` — force re-resolution of the control's ref (refresh the cache).
- `control add-implementation <id> --impl-id <iid> (--ref-file/--ref-anchor |
  --ref-cmd/--ref-dir)` — append an `Implementation` (N:M).
- `control import <manifest> [--format toml|json] [--prefix requirements] [--title-field
  title]` — for each `requirements.<ID>` table, create control `slug(<ID>)` with
  `ref: file_anchor{path:<manifest>, anchor:<prefix>.<ID>.<title-field>}`. IDs are
  lowercased to satisfy the slug rule (`CKSPEC-ARCH-001 → ckspec-arch-001`).
- `process check add <pid> --description <d> --enforcement <e> [--ref-file/--ref-anchor |
  --ref-cmd/--ref-dir]` — ref-backed check.
- `process check <pid> <cid> --pass|--fail` — for a **ref-backed** check this must NOT be
  able to forge a green: **reject** with a new `DomainError::RefBacked { kind, id, fix }`
  ("this check derives its result from its ref; fix the source, then re-resolve") — SC-5.

Rendering: extend `Control`/`Check` `Display` and the JSON view to include `resolution`
(`Derived`), `fallback_title`, `implementations[].resolution`, and the
`asserted|derived|unresolved|stale` distinction. JSON stays a transparent projection
(extend `WithNext`/view structs; no human-only strings in `data`, C5).

### 2.5 Readiness becomes a true truth-meter (`crates/domain/src/readiness.rs`)

The cli computes each control/check's `Derived` (it owns the clock + resolution) and passes
it into `readiness`. Extend, do not rewrite:
- Add a `readiness(store, scope, resolved_index)` signature where `resolved_index:
  &BTreeMap<String, Derived>` is built by the cli (keeps domain clock-free; resolution
  decisions — live vs cached, staleness — made once in cli).
- Split control reporting into **derived** (ref-backed) vs **asserted** (no ref); surface
  as `data.controls.derived` / `data.controls.asserted`.
- `control_coverage`: a ref-backed control counts as implemented-with-evidence **only if**
  it projects to `Derived` + `Pass` (fresh). `Unresolved`/`Stale`/derived-`Fail` ⇒ gap.
- New `gap_findings` kinds: `ref_unresolved`, `ref_stale`, `ref_failing` (each names the
  control/check + source + fix). Wire derived check `fail` through the existing
  `refuting_signals`/`failed_check` path so the verdict is never green over a red source.
- Evidence honesty (DoD bullet): for `EvidenceKind::File`, the cli stats the target and
  passes a bool; readiness flags a `dangling_evidence` gap when absent. Keep minimal
  (file-existence only).

---

## 3. Implementation Checklist (ordered — each step ends green)

> TDD throughout (project rule): write the failing test first, watch it fail, implement,
> watch it pass, keep `just check` green before moving on. Commit per step (conventional
> commits; no `Co-Authored-By` trailer).

1. **Domain types (no behavior change yet).** Add `crates/domain/src/reference.rs` with
   `Ref`, `Outcome`, `value_to_outcome`, `Resolution`, `is_stale`, `Derived`. Unit-test
   `value_to_outcome` (met/unmet/title/garbage) and `is_stale` (boundary, freshness=0).
   Export from `lib.rs`. ⇒ `cargo test -p domain` green; `just check` green.
2. **Additive fields + projection (domain).** Add `ref`, `resolved`, `implementations` to
   `Control`; `ref`, `resolved` to `Check`; add `Implementation`. All `#[serde(default,
   skip_serializing_if=...)]`. Add `display_title`, `effective_status`,
   `effective_result`, `Control::project` (incl. N:M aggregate). Unit tests: honesty (ref
   `fail` ⇒ not implemented even if stored `Implemented`); title fallback; N:M aggregate
   (one fail ⇒ not green); **backward-compat round-trip** (no-ref control serializes
   byte-identical to v0). ⇒ SC-2 partial, SC-5 (domain), SC-10 (domain).
3. **Infra resolver.** Add `toml` to workspace deps; add `serde_json`+`toml` to infra
   Cargo.toml + the allowlist (C4). Add `crates/infrastructure/src/resolver.rs`
   (`resolve_file_anchor`, `run_command`) returning infra-local structs; export from
   `lib.rs`. Tests (infra, `tempfile`): TOML leaf, JSON leaf, missing file, missing
   anchor, non-scalar leaf, command exit 0 / non-zero / spawn-fail. Confirm
   `arch_allowlist`/`infra_imports_domain` still pass. ⇒ `just check` green.
4. **CLI bridge.** Add `cli/src/resolve.rs` (`resolve(&Ref, now) -> Resolution`) and
   `parse_iso_to_epoch` in `cli/store.rs`. No new command yet; pure plumbing + a unit
   test of the bridge mapping.
5. **Control ref commands + rendering.** Extend `control add` with ref flags; add
   `control resolve`; resolve on `show` (live for file_anchor, cached for command),
   populate `Derived`, render both surfaces (`resolution`, `fallback_title`). Integration
   tests in `muster.rs`: SC-3 (title derived + edit reflects), SC-6 (dangling ⇒
   unresolved), SC-7 (command + `MUSTER_FRESHNESS_SECS=0` ⇒ stale).
6. **Check ref commands + honesty enforcement.** Extend `process check add` with ref
   flags; derive `last_result` on `process show`; make `process check --pass/--fail`
   reject ref-backed checks (`DomainError::RefBacked`). Integration: SC-4, SC-5.
7. **Readiness truth-meter.** Add the `resolved_index` param; split derived/asserted
   controls; count unresolved/stale/failing as gaps; wire derived check fails into
   refuting signals; add `ref_unresolved`/`ref_stale`/`ref_failing` gap kinds; optional
   `dangling_evidence`. Update the `readiness.rs` cli handler to build the index.
   Integration: SC-8. Keep all existing readiness tests passing.
8. **N:M implementations.** `control add-implementation`; render per-impl resolution;
   aggregate honesty in `project`. Integration: SC-10 (two impls, one met one unmet ⇒ not
   green).
9. **Reference-import.** New `cli/src/import.rs` + `control import` command; parse the
   manifest (reuse the resolver's parse or a thin cli reader), create controls with
   `file_anchor` refs (not copies). Integration: SC-9 (import temp 3-req TOML; edit a
   title reflects).
10. **ckeletin acceptance.** Integration test in `muster.rs`: copy
    `/Users/peiman/dev/ckeletin-rust/conformance-mapping.toml` into a tempdir, flip one
    `status` to `unmet`, point a control + check at it, assert derived title + refuses
    green. Add one real read-only smoke against the actual path behind an env guard
    (skips when absent) so CI stays offline-safe. ⇒ SC-11.
11. **`explain` + `catalog` + docs.** Update `explain.rs` intents; ensure `catalog`
    enumerates new commands/flags. Update module doc-comments to cite the Manifesto
    principle each new piece serves (#7 on `Ref`, #8 on the bridge, #1 on `is_stale`).
    Correct any now-false v0 statement in `AGENTS.md`/`README.md` (#10 honesty; do not
    duplicate). ⇒ SC-12, SC-13.
12. **Final gate.** `just check` green; re-read SPEC DoD 1–7 against SC-3..SC-11;
    `cargo fmt --all`; confirm no `Co-Authored-By` trailer in commits.

---

## 4. Testing Strategy

- **Domain unit tests (pure, fast, deterministic):** `value_to_outcome`, `is_stale` (incl.
  `freshness_secs=0`), `effective_status`/`effective_result` honesty (stored `Implemented`
  + derived `Fail` ⇒ not implemented), `display_title` fallback, N:M aggregate, and a
  **serde backward-compat** test (deserialize a v0 control JSON literal with no `ref`;
  re-serialize; assert no `ref`/`resolved`/`implementations` keys appear).
- **Infra tests (`tempfile`):** file_anchor over TOML and JSON happy paths; missing file,
  missing/nested-missing anchor, non-scalar leaf (each yields a `reason`, never panics);
  `run_command` exit 0/non-zero/spawn-failure. The `print_stdout/print_stderr=deny` lint
  enforces no infra console output at clippy time.
- **CLI integration (`assert_cmd`, isolated `MUSTER_DATA_DIR`, both surfaces)** — mirror
  the existing `muster.rs` style (the `data(dir, args)` helper that asserts
  `status=="success"` and returns `data`). One test per SC-3..SC-11, each asserting on
  `--output json` `data` fields so a cold agent can verify. Use temp source files for
  determinism; SC-7 sets `MUSTER_FRESHNESS_SECS=0`; SC-11 uses a flipped temp copy of the
  real ckeletin manifest plus an env-guarded read-only smoke.
- **Regression:** run the full pre-existing `muster.rs`/`cli.rs` suites unmodified; any
  change to an existing assertion must be justified as additive, not a weakening.
- **Enforcement (#9):** `arch_allowlist`, `infra_imports_domain`, all violation trybuild
  tests, `ckeletin-check`, and `ckeletin-health` are the automated guard that the layer
  split held — they run inside `just check` and gate every step.
- **Coverage:** keep `just coverage` ≥ 85% (the new domain projection + infra resolver are
  the highest-value lines to cover; the cli bridge is covered by the integration SCs).

---

## 5. Manifesto application (graded — SC-12)

- **#7 Single Source of Truth** — `Ref` stores a *pointer*; title/status are resolved on
  read from the source. muster never copies the source's value into its own ledger
  (SC-3/SC-9 prove an external edit changes muster's output with no muster mutation).
- **#10 Feedback Cycle** — v1 *is* the spec evolving: v0's "recorder" hypothesis was
  refuted by the ckeletin dogfood; v1 inverts to resolve-on-read. The honest `asserted vs
  derived` split keeps muster honest about its own glue thesis.
- **#1 Truth-Seeking / #3 Honest signals** — `Unresolved`/`Stale` are surfaced, never
  silently green; every error names the offending path/anchor and the fix.
- **#8 Separation of Concerns** — pure `Ref`/`Resolution`/projection in domain; fs/process
  I/O in infrastructure (which cannot even see domain); cli bridges. Compiler/test enforced.
- **#9 Automated Enforcement** — the honesty rule is enforced by `effective_*` deriving on
  read (not by convention), and the layer split by the allowlist + trybuild tests.
- **#4 Lean Iteration / out-of-scope discipline** — exactly the P0+P1 organs, two resolver
  kinds, no network/`url`, no LLM, no UI, no re-implementation of ckeletin's engine.

```json
{"rationale": "A v1 plan that inverts muster from recorder to glue via a pure domain Ref/Resolution + projection, an infra resolver that (forced by the no-domain-dep rule) returns infra-local results bridged in cli, derived honest control/check status with Unresolved/Stale states, N:M implementations, reference-import, and a read-only ckeletin acceptance — each SC mechanically verifiable with just check kept green.", "evidence": {"files": ["PLAN.md"]}}
```
