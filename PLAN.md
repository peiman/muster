# PLAN.md — muster v0

> Build **muster** v0 exactly per `SPEC.md`, scaffolded from the **ckeletin-rust**
> framework at `/Users/peiman/dev/ckeletin-rust`, developed by **Peiman's Manifesto**
> (10 principles, verbatim in `SPEC.md` §"How to build this"). The validator grades
> manifesto-alignment, not just feature presence. Rust; no network, no database, no
> async runtime.

This plan is authoritative for the Executor. Entity field schemas, the command
surface, and the Definition-of-Done walkthrough live in `SPEC.md` — **reference
them, do not re-copy them** (Manifesto #7 SSOT). Where this plan and `SPEC.md`
disagree, `SPEC.md` wins; raise a revision note rather than guessing.

### What changed since the rejected plan (re-architecture note, #10)

The prior plan's Step 0 assumed a copy of the ckeletin scaffold was **"already
present in this repo root"** and only instructed the *rename*. That premise is
false: `repos/muster/` contains only `SPEC.md`, `PLAN.md`, `README.md`, and
`.workhorse/` — **no `crates/`, no `Cargo.toml`, no `ckeletin-project.toml`, no
`Justfile`.** Running `just init` here would have failed (no Justfile), and a
naive "find the scaffold and init it" recovery risks the executor running
`init.sh` inside the **shared source framework** `/Users/peiman/dev/ckeletin-rust`
— which is destructive: `init.sh` (verified) operates on **relative paths** and
its step 7 runs `rm -rf .git && git init`, permanently renaming and re-historying
whatever directory is CWD.

The new Step 0 (and the §2.0 bootstrap contract) makes the copy **explicit**,
**hard-asserts CWD == `repos/muster`** before any init invocation, and forbids
touching the source tree. This is the structural fix; the remainder of the plan
(SC↔DoD mapping, layering, no-network deps, readiness design, manifesto binding)
is retained.

---

## 1. Success Criteria (mechanically verifiable by the Reviewer)

Each criterion is a command sequence the Reviewer can run from a clean checkout
with an isolated data dir. The Reviewer isolates state by exporting
`MUSTER_DATA_DIR=$(mktemp -d)/.muster` before each scenario. All `--output json`
assertions are against the framework envelope `{status, command, data, error}`.

**SC-0 — Scaffold provenance (the re-architected bootstrap).** From `repos/muster`,
the project is a self-contained ckeletin-derived Rust workspace: `crates/{domain,
infrastructure,cli}/`, `Cargo.toml`, `ckeletin-project.toml`, `Justfile`, and
`.ckeletin/` all exist at the repo root. The original `SPEC.md` and `PLAN.md` are
preserved (byte-identical content to before scaffolding, modulo the README). The
**source framework `/Users/peiman/dev/ckeletin-rust` is untouched** — its
`Cargo.toml` still contains the upstream slug `peiman/ckeletin-rust` and its
`crates/cli/Cargo.toml` still says `name = "ckeletin-rust"`. (Reviewer asserts both:
the derived tree is renamed; the source tree is not.)

**SC-1 — Builds green.** `just check` exits `0` (fmt, clippy deny-list, cargo-deny,
full test suite, ckeletin-health) on a clean tree. No `print_stdout`/`print_stderr`
in domain/infrastructure. The binary is named `muster`; `MUSTER_` is the env prefix;
no residual `ckeletin-rust` strings in shipped crates (`grep -r ckeletin-rust crates/`
is empty except the intentionally-kept framework re-export shim in
`crates/infrastructure`, which depends on the `ckeletin` crate by design).

**SC-2 — Init + structured process (DoD #1).**
`muster init` exits `0` and creates the data dir.
`muster process add incident-mgmt --name "Incident Management" --owner ciso`
exits `0`; the created process has `status: "proposed"` (default).
`muster process show incident-mgmt --output json` returns
`data.id=="incident-mgmt"`, `data.name=="Incident Management"`, `data.owner=="ciso"`,
`data.status=="proposed"`, and arrays (`steps`, `controls`, `risks`, `metrics`,
`checks`, `revisions`, `evidence`) present as JSON arrays (possibly empty).

**SC-3 — Recursive graph + tree (DoD #2).**
`muster process add containment --name Containment` then
`muster process step add incident-mgmt --description "Contain" --process-ref containment`
succeeds. `muster process show incident-mgmt --tree --output json` shows the step
with `process_ref=="containment"` AND an expanded sub-process node for
`containment`. Human `--tree` renders containment nested/indented under the step.

**SC-4 — Controls + linking + evidence (DoD #3).**
`muster control add a5-24 --title "Incident planning" --clause-ref "ISO 27001 A.5.24"`,
`muster process link-control incident-mgmt a5-24`,
`muster control set-status a5-24 implemented`,
`muster control attach-evidence a5-24 note "runbook approved"` all exit `0`.
`muster control show a5-24 --output json` → `status=="implemented"`, `clause_ref`
set, `evidence` length `>=1`. `incident-mgmt.controls` contains `"a5-24"`.

**SC-5 — Conformance check ingest, the #9 seam (DoD #4).**
`muster process check add incident-mgmt --description "runbook exists in CI" --enforcement ci`
creates a check with `last_result=="unknown"`, `enforcement=="ci"`.
`muster process check incident-mgmt <check-id> --pass` sets `last_result=="pass"`
and a non-empty `last_run_ts`. `--fail` sets `"fail"`. Invalid enforcement value is
rejected with a non-zero exit and a stderr message naming the allowed values.

**SC-6 — Incident command & control (DoD #5).**
`muster incident report inc-1 --title "Outage" --severity high --process incident-mgmt`
→ `status=="open"`, `severity=="high"`, `process_ref=="incident-mgmt"`.
`muster incident log inc-1 "contained"` appends one timeline entry (`log` length
grows by 1, each entry has `ts` + `note`). `muster incident close inc-1` →
`status=="closed"`.

**SC-7 — Nonconformity + the #10 feedback cycle (DoD #6).**
`muster nonconformity raise nc-1 --from-incident inc-1 --description "detection too slow"`
→ `source=="incident"`, `process_ref=="incident-mgmt"` (copied from the incident),
`status=="open"`. `muster process revise incident-mgmt "tightened detection step" --because nc-1`
appends a `revisions[]` entry with `summary` set, `because=="nc-1"`, and a `ts`.
`muster nonconformity resolve nc-1 --corrective-action "added automated alert"` →
`status=="closed"`, `corrective_action` set.

**SC-8 — Readiness is an honest truth-meter (DoD #7).**
After `muster process set-status incident-mgmt active`,
`muster readiness --output json` returns `data` containing, at minimum, these keys
(exact names enforced by tests): `verdict`, `control_coverage`
(`{applicable, implemented_with_evidence, percent, gaps[]}`), `proven` (list of
active process ids), `asserted` (list), `refuting_signals[]`, `enforcement[]`
(per-process strongest enforcement), `gap_findings[]`, `cycles[]`.
Truthfulness assertions:
- After `nc-1` is resolved and `inc-1` closed, `refuting_signals` contains no entry
  caused by `nc-1`/`inc-1` for `incident-mgmt`.
- `enforcement` for `incident-mgmt` reports strongest == `"ci"`.
- `control_coverage.percent` equals `implemented_with_evidence / applicable * 100`
  computed over applicable controls; `a5-24` (implemented + evidence) counts as
  covered.
- `verdict` is `"READY"` only when zero gap findings, zero open refuting signals,
  and 100% applicable-control coverage; otherwise it is the string `"GAPS: <n>"`
  with `n == gap_findings.length`. It is **never** `READY` while any gap exists.

**SC-9 — Readiness moves correctly (DoD #7, Manifesto #1).** Mutating state changes
the numbers: e.g. adding a second applicable control with no evidence lowers
`control_coverage.percent` and adds it to `control_coverage.gaps`; resolving an
open nonconformity removes its refuting signal and may flip `verdict`. A test
asserts a before/after delta in at least two distinct readiness fields.

**SC-10 — Cycle detection terminates (DoD #8).** Introduce a cycle (process A step
→ B, process B step → A). `muster readiness --output json` completes (exit within
the test timeout, no hang) and `data.cycles` contains the cycle (a list of process
ids). `muster process show A --tree` also terminates and marks the cycle instead of
recursing infinitely.

**SC-11 — Dual-surface parity (DoD #9, Manifesto #7).** For every command, the
`--output json` `data` object contains **every fact** the human rendering shows, as
structured fields — no human-only strings, no markdown/pre-rendered tables embedded
in JSON. A parity test drives a representative command in both modes and asserts the
human text's facts are each present as JSON fields. Output ordering is deterministic
(entities/lists sorted by id, steps by `n`) so agents can diff/assert.

**SC-12 — Honest signals & guidance (Manifesto #3, #5, #6).** Exit code is `0` only
on success. Not-found errors name the offending id **and** the corrective command
(e.g. `process 'foo' not found — create it with: muster process add foo --name …`).
`muster explain` maps intents → commands. Every command suggests a natural next
action after success (human surface). `muster --help` and per-subcommand `--help`
work without a manual.

**SC-13 — Coverage gate.** `just coverage` reports **>= 85%** line coverage
(framework threshold). Domain readiness and cycle-detection logic are unit-tested
directly.

---

## 2. Architecture

### 2.0 Scaffold bootstrap contract (the re-architected load-bearing step)

**Verified facts about the framework** (read directly from
`/Users/peiman/dev/ckeletin-rust`):
- `just init <name>` invokes `.ckeletin/scripts/init.sh <name> <force>`.
- `init.sh` operates entirely on **relative paths** (`crates/cli/Cargo.toml`,
  `Cargo.toml`, `crates/cli/src/root.rs`, `crates/cli/src/main.rs`,
  `crates/domain/src/ping.rs`, `crates/cli/tests/cli.rs`) — it acts on **CWD**, not
  on any argument-supplied directory.
- It guards against re-init by requiring the upstream slug `peiman/ckeletin-rust`
  in `Cargo.toml` (present only in an un-renamed scaffold) unless `force=true`.
- It refuses to run with uncommitted changes in a non-interactive shell **unless
  `CKELETIN_ASSUME_YES=1`**.
- **Step 7 is destructive: `rm -rf .git && git init && git commit` in CWD.** Running
  it in the wrong directory renames and re-historys that directory.

**Therefore the bootstrap is an explicit, fenced sequence (Executor must follow
exactly):**

1. **Assert CWD.** The Executor's CWD for the entire run is the project root
   `repos/muster`. Before doing anything, assert it:
   ```bash
   test "$(basename "$PWD")" = muster && test -f SPEC.md && test -f PLAN.md \
     || { echo "FATAL: not in repos/muster"; exit 1; }
   ```
   If this fails, STOP and raise a revision note. Never `cd` into
   `/Users/peiman/dev/ckeletin-rust` to run any `just`/`init` recipe.

2. **Copy the framework in (non-destructive of the spec docs).** Mirror the source
   framework into the project root, **excluding** the source's git history, build
   artifacts, and OS cruft, and **without** `--delete` so `SPEC.md`, `PLAN.md`, and
   `.workhorse/` are preserved:
   ```bash
   rsync -a \
     --exclude='.git/' --exclude='target/' --exclude='.DS_Store' \
     /Users/peiman/dev/ckeletin-rust/ ./
   ```
   (Trailing slashes are load-bearing: copies the *contents* of the source into
   `./`.) After this, `Cargo.toml`, `ckeletin-project.toml`, `Justfile`,
   `crates/`, and `.ckeletin/` exist at the project root; the source framework's
   README replaces muster's placeholder README (rewritten in Step 6 anyway).

3. **Rename via the framework's own init, in this CWD.** With CWD still
   `repos/muster` (re-assert per Step 1), the slug is present (fresh copy) so the
   guard passes; uncommitted changes exist (this repo already had a `.git`), so set
   the bypass:
   ```bash
   CKELETIN_ASSUME_YES=1 just init muster
   ```
   This renames the binary to `muster`, sets the env prefix `MUSTER_`, rewrites
   `ckeletin-rust` strings in the cli crate, compiles all targets, **and resets git
   history in `repos/muster`** to a single "Initial scaffold from ckeletin-rust"
   commit. That git reset is **expected and correct** — the muster project becomes a
   clean ckeletin-derived repo. It only ever touches `repos/muster/.git`, never the
   source framework's, because CWD is asserted.

4. **Verify provenance both ways (SC-0):** `grep -q 'peiman/ckeletin-rust'
   /Users/peiman/dev/ckeletin-rust/Cargo.toml` MUST still succeed (source untouched),
   and `grep -q 'name = "muster"' crates/cli/Cargo.toml` MUST succeed (derived tree
   renamed).

If `just init` is somehow unavailable post-copy, the executor performs the
equivalent renames manually **in `repos/muster`** per `init.sh` (binary name, env
prefix, string replacements, README/CHANGELOG reset) and verifies SC-0/SC-1 — but
must still never run any rename inside the source framework.

### 2.1 Layering (Manifesto #8 Separation of Concerns)

Reuse the ckeletin three-crate split exactly; add no new crate.

| Crate | Allowlist (ckeletin-project.toml) | Responsibility for muster |
|-------|-----------------------------------|---------------------------|
| `crates/domain` | `["serde", "thiserror"]` *(currently `["serde"]`; add `thiserror`)* | **Pure** entity types, enums, validation, the in-memory aggregate (`Store`), graph traversal (tree + cycle detection), and `readiness` computation. No I/O, no clap, no fs. Every output type derives `Serialize` + implements `Display`. |
| `crates/infrastructure` | `["ckeletin"]` *(unchanged)* | Re-export shim only (`output`, `config`, `logging`, `catalog`, `build_info`, `project_config`). Untouched. |
| `crates/cli` | *(no allowlist — top layer)* | clap command tree, dispatch, per-command handlers, the **persistence layer** (`store.rs` — file-per-entity JSON I/O), the `explain` intent map, and rendering via `infrastructure::output::Output`. May depend on `serde`, `serde_json`, `thiserror`, `clap`. |

**Why persistence sits in `cli`, not `domain` or `infrastructure`:** domain must stay
side-effect-free (#8); `infrastructure` must not import `domain` (its allowlist is
`ckeletin` only). The cli layer owns the disk boundary: it loads the on-disk JSON
files into the domain `Store` aggregate, calls a pure domain operation, then
persists. This is the only deliberate deviation from "infra owns I/O" and is
recorded here per #10. The domain allowlist gains `thiserror` so domain can define
typed validation errors without hand-rolling `std::error::Error`. Keep `just check`
green after the edit.

### 2.2 Data model & persistence (process is the spine)

Entity schemas are defined in `SPEC.md` §"Data model" — implement them verbatim:
`Process` (with nested `Step`, `Check`, `Revision`), `Control`, `Incident`,
`Nonconformity`, `Evidence`. Enums (`Status`/process lifecycle, control `status`,
`Severity`, incident `status`, nonconformity `source`/`status`, check `Enforcement`,
`last_result`) are Rust enums with `#[serde(rename_all = "snake_case")]` and a
`clap::ValueEnum` impl (or `FromStr`) so they parse from CLI args and reject invalid
values with a clear error.

**On-disk layout** (git-diffable, one JSON file per entity; Manifesto #4 minimal):
```
<data-dir>/                 # default ./.muster ; overridable via MUSTER_DATA_DIR
  manifest.json             # {schema_version} marker written by `init`
  processes/<id>.json
  controls/<id>.json
  incidents/<id>.json
  nonconformities/<id>.json
```
- **Data-dir resolution (SSOT, one function in `store.rs`):** `MUSTER_DATA_DIR` env
  var if set, else `./.muster`. Tests set the env var to a temp dir for isolation.
- `store.rs` exposes `Store::load(dir) -> Result<domain::Store, StoreError>` (reads
  all four entity dirs into the in-memory aggregate) and
  `save(dir, &domain::Store)` (writes each entity to its `<id>.json`, pretty,
  stable key order). Commands: load → call domain op → save → render.
- **Not initialized:** if the data dir / manifest is absent, every command except
  `init`/`explain` fails non-zero with `store not initialized — run: muster init`.

### 2.3 Domain operations (pure, in `domain`)

The aggregate `domain::Store { processes, controls, incidents, nonconformities }`
holds `BTreeMap<String, _>` (id-sorted → deterministic). Domain functions are pure
and total; they return `Result<_, DomainError>`:
- Mutators: `add_process`, `set_process_status`, `add_step`, `link_control`,
  `add_risk`, `add_metric`, `add_check`, `ingest_check(pass|fail, ts)`,
  `revise(summary, because, ts)`, `attach_process_evidence`; `add_control`,
  `set_control_status`, `attach_control_evidence`; `report_incident`,
  `log_incident(note, ts)`, `close_incident`; `raise_nonconformity` (with
  `--from-incident` copying `process_ref`), `resolve_nonconformity`.
  Each validates: slug format `^[a-z][a-z0-9-]*$`, uniqueness, referential
  existence (`--process-ref`, `--from-incident`, `link-control` targets), enum
  membership. Timestamps (`ts`) are passed **in** from the cli boundary (domain
  stays pure/deterministic; do not call the clock inside domain).
- Read/graph: `show(id)`, `show_tree(id)` (depth-first expansion of `step.process_ref`
  with a visited-set; on revisit emit a `cycle` marker node and stop — never
  recurse infinitely), `list_*` (id-sorted).
- `detect_cycles() -> Vec<Vec<String>>`: directed graph, nodes = processes, edges =
  `process → step.process_ref`. Iterative/colored DFS (white/grey/black); a grey
  re-visit is a back-edge → record the cycle. Always terminates.

### 2.4 `readiness` (the headline; pure domain function)

`readiness(store, scope: Option<&str>) -> Readiness`. Scope `--process <id>`
restricts to that process and its reachable sub-graph (via `process_ref`,
cycle-safe). `Readiness` is one `#[derive(Serialize)]` struct; fields per **SC-8**.
Computation rules (Manifesto #1 truth, #2 refuting signals, #9 ladder, #10):
- **control_coverage**: `applicable` = controls with `applicable==true` (when scoped,
  restrict to controls referenced by the process sub-graph: `process.controls[]` ∪
  each `step.controls[]`, recursively, ∩ applicable). `implemented_with_evidence` =
  those with `status==implemented` **and** `evidence.len()>=1`. `percent` =
  `implemented_with_evidence / applicable * 100` (0 applicable → 100%). `gaps[]` =
  applicable controls not implemented-with-evidence (id + reason).
- **proven vs asserted** (active processes only): `proven` = has `>=1` evidence
  **and** no open incident referencing it, no open nonconformity referencing it, no
  check with `last_result==fail`. Else `asserted`.
- **refuting_signals[]**: processes carrying an open incident, an open
  nonconformity, or a failed last check — each entry names the process and the
  signal source.
- **enforcement[]**: per process, strongest enforcement among its checks on the
  ladder `compile_time > lint > script > ci > honor` (#9). A process whose strongest
  is `honor`, or that has no checks, is flagged (`honor_only` / `no_enforcement`).
- **gap_findings[]** (guide-don't-gate, #4): active process with empty `risks`;
  empty `metrics`; empty `controls`; a control `implemented` but evidence-less; an
  open nonconformity with empty `corrective_action`; each `cycle`. Every finding is
  `{kind, subject_id, message}`.
- **verdict**: `"READY"` iff `gap_findings` empty AND no open refuting signals AND
  `control_coverage.percent == 100`. Otherwise `"GAPS: <n>"`, `n =
  gap_findings.len()`. Never green while a gap exists (#3 honest signals).

### 2.5 CLI surface (clap derive, matches SPEC §"Command surface" verbatim)

Extend `crates/cli/src/root.rs` `Commands` with nested `#[derive(Subcommand)]`
groups: `Init`, `Explain`, `Process(ProcessCmd)`, `Control(ControlCmd)`,
`Incident(IncidentCmd)`, `Nonconformity(NonconformityCmd)`, `Readiness(ReadinessArgs)`.
Keep the existing global `--output text|json` flag (the dual-surface engine) and
`version`/`catalog`. (Replace/extend the scaffold's kept `ping` command — do not
leave an empty `Commands` enum, which the entry point cannot match exhaustively.)

**The one tricky case — `process check`** has two forms in `SPEC.md`:
`process check add <id> --description … --enforcement …` (create) and
`process check <id> <check-id> --pass|--fail` (ingest, no verb). Model it cleanly
with clap's subcommand-or-args idiom so **both literal SPEC invocations work**:
```rust
#[derive(clap::Args)]
#[command(args_conflicts_with_subcommands = true, subcommand_negates_reqs = true)]
struct CheckArgs {
    #[command(subcommand)] sub: Option<CheckSub>,   // `add`
    id: Option<String>,                              // ingest: process id
    check_id: Option<String>,                        // ingest: check id
    #[arg(long)] pass: bool,
    #[arg(long)] fail: bool,
    #[arg(long, num_args = 2, value_names = ["KIND","VALUE"])] evidence: Option<Vec<String>>,
}
#[derive(clap::Subcommand)] enum CheckSub {
    Add { id: String, #[arg(long)] description: String, #[arg(long)] enforcement: Enforcement },
}
```
Other nested verbs (`process step add`, `process risk add`, `process metric add`)
are ordinary nested subcommands. `--pass` and `--fail` are mutually exclusive
(`conflicts_with`); requiring exactly one is validated in the handler with an honest
error if neither/both is given.

**Dispatch:** mirror the existing `main.rs` pattern — add arms to the dispatch
`match` and to `subcommand_name()` (so error envelopes carry the right command
name). Handlers take `&Output`; render via `output.success(cmd, &result, out)` /
`output.message(...)` / `output.error(...)`. Every result type is a domain
`Serialize + Display` struct; `Display` is the human surface (incl. the suggested
next action), serde is the JSON surface — same facts (#7).

**`explain`** is a new cli-owned command: a static, deterministic intent→command map
(`Serialize + Display`), e.g. "Stand up a process → muster process add …",
"Prove a control → control set-status + attach-evidence", "See where you stand →
muster readiness". Both surfaces.

### 2.6 Manifesto binding (the validator grades this)

- #1 Truth / #2 Curiosity: readiness reports *proven-vs-asserted* honestly and
  surfaces refuting signals; never paints green over gaps.
- #9 Automated Enforcement: per-process enforcement strength recorded on the ladder;
  honor-only flagged. The check-ingest seam (`process check … --pass/--fail`) is the
  CI plugin's future entry point (#5 platform, not feature).
- #10 Feedback Cycle: `revisions[]` is a first-class, append-only, auditable artifact
  with `--because` linking the refuting signal that drove the change.
- #7 SSOT: one data-dir resolver, one output-mode resolver (framework), schemas
  referenced from SPEC not duplicated.

---

## 3. Implementation Checklist (ordered; Executor follows exactly)

**Step 0 — Bootstrap the scaffold into `repos/muster` (per §2.0; the re-architected
step).**
1. **Assert CWD** is `repos/muster` (`basename "$PWD" == muster` AND `SPEC.md` +
   `PLAN.md` present). If not, STOP — do not proceed, do not `cd` into the source
   framework.
2. **Copy** the framework in with the exact exclude set (non-destructive of spec
   docs):
   `rsync -a --exclude='.git/' --exclude='target/' --exclude='.DS_Store' /Users/peiman/dev/ckeletin-rust/ ./`
3. **Re-assert CWD**, then rename via the framework's own init:
   `CKELETIN_ASSUME_YES=1 just init muster` (renames binary→`muster`, env→`MUSTER_`,
   rewrites cli strings, compiles all targets, resets `repos/muster/.git` to a clean
   scaffold commit — expected).
4. **Verify provenance (SC-0):** source `/Users/peiman/dev/ckeletin-rust/Cargo.toml`
   still has `peiman/ckeletin-rust`; derived `crates/cli/Cargo.toml` has
   `name = "muster"`. `grep -r ckeletin-rust crates/` is empty except the
   infrastructure re-export shim.
5. `just check` must be green before writing any feature code. Commit baseline.

**Step 1 — Allowlist & deps.**
6. Edit `ckeletin-project.toml`: change `domain = ["serde"]` →
   `domain = ["serde", "thiserror"]` (with a justifying comment per the file's
   convention).
7. `crates/domain/Cargo.toml`: add `thiserror = { workspace = true }`.
   `crates/cli/Cargo.toml`: add `serde`, `serde_json`, `thiserror` (workspace deps).
   Run `just check` (the architecture allowlist test must pass with the new
   allowlist).

**Step 2 — Domain types (TDD: write the type + unit test together).**
8. `crates/domain/src/model.rs` (or a `model/` module): all entities + enums per
   `SPEC.md`. Derive `Serialize, Deserialize, Debug, Clone, PartialEq`; enums get
   `#[serde(rename_all="snake_case")]`. Implement `Display` for each
   command-result/view type. Add `DomainError` (thiserror) covering: invalid slug,
   duplicate id, not-found(kind,id), bad enum value, missing reference, ambiguous
   pass/fail.
9. `domain::Store` aggregate (`BTreeMap`s) + `Default`. Declare modules in `lib.rs`.

**Step 3 — Domain operations + graph + readiness (each with unit tests).**
10. Implement all mutators/readers from §2.3 with validation. Unit-test slug
    validation, uniqueness, referential checks, `raise --from-incident` copying
    `process_ref`, and `revise` appending with `because`.
11. Implement `show_tree` (cycle-safe) and `detect_cycles`; unit-test a 2-node cycle
    terminates and is reported.
12. Implement `readiness` per §2.4. Unit-test: coverage math, proven-vs-asserted,
    refuting-signal removal after resolve/close, enforcement ladder ranking, verdict
    never-green-with-gaps, and a before/after delta (SC-9).

**Step 4 — Persistence (`crates/cli/src/store.rs`).**
13. Data-dir resolver (`MUSTER_DATA_DIR` else `./.muster`). `init` creates dirs +
    `manifest.json`. `load`/`save` for the four entity dirs (serde_json pretty,
    id-sorted). `StoreError` (thiserror) with not-initialized + io variants, each
    carrying a fix message.

**Step 5 — CLI wiring (`root.rs`, `main.rs`, handlers).**
14. Add the clap command tree per §2.5 (incl. the `process check` subcommand-or-args
    idiom). Add dispatch arms + `subcommand_name()` entries. Replace/extend the kept
    `ping` command.
15. One handler module per group (`process.rs`, `control.rs`, `incident.rs`,
    `nonconformity.rs`, `readiness.rs`, `init.rs`, `explain.rs`). Pattern: resolve
    data dir → `store::load` → call domain op (pass `ts` from the boundary clock for
    log/revise/check-ingest) → `store::save` → `output.success(...)`. On domain/store
    error → non-zero exit + `output.error` naming id + corrective command. Each
    success `Display` ends with a "Next:" suggestion (#6 hand the next action).
16. Implement `explain` intent map and ensure `--help` text is meaningful for every
    command (clap `about`/`long_about`).

**Step 6 — Tests & verification (separate authoring vs review pass).**
17. Write integration tests in `crates/cli/tests/cli.rs` (assert_cmd) — see §4.
18. `just check` green; `just coverage` >= 85%. Fix gaps. Final scaffold-scan clean
    (SC-1) and provenance check (SC-0).
19. Update `README.md` to muster (one-paragraph: what it is + `muster explain`).

---

## 4. Testing Strategy

**Layered (Manifesto #8), evidence-driven (#1).**

0. **Bootstrap provenance check (SC-0)** — a shell assertion run by the Reviewer (not
   a Rust test): derived tree renamed, source framework untouched, no stray
   `ckeletin-rust` in `crates/` beyond the infra shim.

1. **Domain unit tests** (in-file `#[cfg(test)]`, pure, no I/O — fast and total):
   - Validation: slug regex accept/reject; duplicate-id rejected; not-found and
     missing-reference errors; enum parse rejects garbage.
   - Graph: `detect_cycles` finds a 2-node and a 3-node cycle and **terminates**;
     `show_tree` on a cyclic graph terminates with a cycle marker.
   - readiness: coverage percentage math (incl. 0-applicable → 100%); proven vs
     asserted transitions; refuting signal appears then disappears after
     resolve/close; enforcement ladder picks the strongest; `verdict` is `GAPS: n`
     whenever any gap exists and `READY` only at zero-gap/100%-coverage; SC-9 delta.

2. **CLI integration tests** (`crates/cli/tests/cli.rs`, `assert_cmd`, each test in
   its own `MUSTER_DATA_DIR` temp dir, `parse_json_stdout` helper):
   - **`dod_full_spine`**: scripts DoD steps 1→7 from `SPEC.md` end-to-end in JSON
     mode, asserting the SC-2…SC-8 facts at each stage (this single test is the
     validator's spine).
   - **`readiness_moves`**: SC-9 — assert before/after deltas in two fields.
   - **`cycle_terminates`**: SC-10 — build a cycle, run `readiness` and
     `process show --tree`; both succeed within the default test timeout and report
     the cycle (a hang fails the test).
   - **`dual_surface_parity`**: SC-11 — run `process show` and `readiness` in both
     modes; assert every fact in the human text exists as a JSON field; assert JSON
     contains no markdown/table strings.
   - **`honest_errors`**: SC-12 — unknown id, missing required flag, bad enum,
     command before `init` → non-zero exit, stderr (human) / error envelope (json)
     naming the offender **and** the fix.
   - **`determinism`**: list commands and readiness emit stable ordering across two
     runs (byte-identical JSON for unchanged state).

3. **Architecture/conformance tests** (inherited from ckeletin, must stay green):
   allowlist enforcement (domain=`serde,thiserror`; infra=`ckeletin`), framework
   purity, violation drift guard. Run via `just check`.

4. **Gate:** `just check` (fmt + clippy deny + cargo-deny + tests + health) exits `0`
   and `just coverage` >= 85% is the definition of "done" for the Reviewer.

---

```json
{"rationale": "Re-architects the rejected plan's false 'scaffold already present' premise into an explicit, CWD-asserted bootstrap (§2.0/Step 0): rsync-copy /Users/peiman/dev/ckeletin-rust into repos/muster excluding .git/target while preserving SPEC/PLAN, then run CKELETIN_ASSUME_YES=1 just init muster only after asserting CWD==repos/muster — since init.sh acts on relative paths and does rm -rf .git, this prevents the executor from renaming or re-historying the shared source framework, verified by the new SC-0 provenance check.", "evidence": {"files": ["PLAN.md"]}}
```
