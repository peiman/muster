# ckeletin Framework Changelog

## [0.2.27] - 2026-06-10

### Added
- **`just ckeletin-outdated`** — reports direct dependencies with newer
  published versions via `cargo outdated --root-deps-only --workspace`
  (graceful install hint when the tool is absent; informational, always
  exit 0, not part of `just check`). Parity with ckeletin-go's
  `task ckeletin:check:deps:outdated`. The scaffold root Justfile gains an
  `outdated` alias (mirrors go's root `check:deps:outdated`). `cargo-outdated`
  added to `ckeletin-doctor`'s tools report (text + JSON). Honest scope note:
  cargo deps only — CI tool pins in ci.yml remain a hand-reviewed surface.

## [0.2.26] - 2026-06-10

### Changed

- **Rust edition bumped from 2021 to 2024** across all framework crates
  (`.ckeletin/crate`, `.ckeletin/conform`) and the workspace root (which
  propagates to `crates/domain`, `crates/infrastructure`, `crates/cli` via
  `edition.workspace = true`). The migration required zero mechanical code
  changes (all five `cargo fix --edition` runs reported no idiom rewrites
  needed); two `collapsible_if` Clippy lints in `scaffold_scan.rs` and
  `conform/src/main.rs` were resolved using the now-stable let-chain syntax
  (`if cond && let Ok(x) = …`).

  **Consumer note:** consumers' own crates keep their own edition — only the
  vendored `.ckeletin/crate` and `.ckeletin/conform` editions change.
  `just check` compiles `.ckeletin/crate` at edition 2024 (requires
  `rustc >= 1.85`), and the framework code now uses let-chains (stable since
  1.88) — so the effective consumer toolchain floor is `rustc >= 1.88`. The
  scaffolded `rust-toolchain.toml` pin is 1.96, so existing consumers already
  satisfy this.

## [0.2.25] - 2026-06-10

### Fixed
- **Scaffold-leftover guard now scans the REAL tree** (Finding 5,
  consumer-feedback-2026-06-10.md). 0.2.24 shipped the guard with 14 call
  sites — all hermetic fixtures plus the fresh-init copy — so an EXISTING
  consumer's tree was never scanned and `just check` stayed green on real
  leftovers (ioguard proved it: green at 0.2.24 with the v0.1.0-eating
  `target/release/ckeletin-rust` still in its release.yml). New test
  `this_repos_real_tree_has_no_scaffold_leftovers` mirrors arch_allowlist's
  real-tree pattern: skips on upstream, red-gates consumers until clean.
  Verified by re-attack: a simulated consumer with a planted leftover fails
  `just check` with file:line messages.

## [0.2.24] - 2026-06-10

### Fixed

- **Instance 4 — `ckeletin-sbom` recipe derives bin package from `cargo metadata`**
  instead of hardcoding `crates/cli/cli.cdx.json`. Consumers with any CLI crate
  name now get a working SBOM recipe without a recipe override. (ioguard had to
  override this recipe because the hardcoded path exited 1 in CI.)

- **Instance 3 — `release.yml` selects the bin package structurally**, not by a
  literal package name (`"cli"` → `select(.targets[]? | .kind[]? == "bin")`).
  Both the tag-reconciliation step and the binary-name resolution step now find
  the correct package regardless of what the consumer calls their CLI crate.

### Added

- **Scaffold-leftover guard** (`scaffold_scan.rs` + `scaffold_leftover_guard.rs`
  fixture tests). Consumer repos now fail `just check` when functional files
  (`.github/workflows/*.yml`, root `Justfile`, `lefthook.yml`, `Cargo.toml`s,
  `deny.toml`) still contain the literal `ckeletin-rust` on non-comment,
  non-gating lines. Exclusions: lines starting with `#`, lines containing
  `github.repository ==`, everything under `.ckeletin/`.

- **`init_smoke` gate**: after the init flow succeeds, the scaffold-leftover scan
  is run against the initialized project in consumer mode. A new scaffolded file
  that bakes in the identity without substitution or derivation becomes a
  PR-time compile failure in this repo — the leftover class is unshippable.

### Migration note for existing consumers

On updating to this version, `just check` may flag your `release.yml` if it
still selects the bin package by the literal name `"cli"` (or hardcodes
`ckeletin-rust`). This is the exact defect that caused ioguard v0.1.0 to publish
zero release artifacts — the binary path in the workflow named the scaffold's
binary, not ioguard's. The fix is to replace the literal-name selection with the
structural form shown in the current upstream `release.yml`:

```yaml
BIN=$(cargo metadata --format-version 1 --no-deps \
  | jq -r '.packages[] | select(.targets[]? | .kind[]? == "bin") | .targets[] | select(.kind[] == "bin") | .name' \
  | head -1)
```

ioguard PR #4 is the worked fix; the current upstream `release.yml` is the
reference implementation.

## [0.2.23] - 2026-06-10

### Added
- **Empty-layer-list semantics pinned by test** — `infrastructure = []` (or any
  empty layer list) in `ckeletin-project.toml` means "this architecture has no
  such layer: enforce nothing for it", while declared layers stay enforced.
  Two consumers (agent-chat, ioguard) ship this shape for adapter crates that
  import the core by design; `empty_layer_list_is_an_honest_declaration_not_an_error`
  in `consumer_layout_fixtures.rs` guards it against future refactors.

## [0.2.22] - 2026-06-10

### Added
- **`ckeletin-project.toml` — project-owned conformance config** (root file,
  survives `just ckeletin-update`).  Consumers declare their layer crate paths,
  per-layer dependency allowlists, and violation test preferences here.  This
  is the ONLY place to tailor conformance; never edit `.ckeletin/crate/tests/`
  files (they are replaced on every update).

  Schema:
  - `[layers]` — `domain`, `infrastructure`, `cli` as single strings or lists
    (multi-crate domain is supported: `domain = ["chat-core", "chat-models"]`).
  - `[allowlists]` — complete `[dependencies]` allowed per layer.  Scaffold
    defaults: `domain = ["serde"]`, `infrastructure = ["ckeletin"]`.
    Use TOML comments above each entry as the justification mechanism.
  - `[violation_tests]` — `enabled = true/false`, optional `domain_dirs` /
    `infra_dirs` for non-scaffold layouts.

- **`project_config` module** (`.ckeletin/crate/src/project_config.rs`): pure
  loader for `ckeletin-project.toml` using figment's Toml provider (no new
  deps — figment was already in `[dependencies]`).  Returns one of three
  outcomes: `Explicit` (file present), `ScaffoldDefaults` (file absent but
  `crates/domain` present), `Absent` (neither — callers skip loudly).
  Malformed TOML is a hard error, never silenced.

- **Behavior matrix in `arch_allowlist.rs` and `violation_drift_guard.rs`**:
  both tests now load from `project_config::load()` instead of hardcoding the
  scaffold layout.  A consumer with custom layer names and a missing
  `ckeletin-project.toml` gets a loud skip nudge — not a panic, not a false
  failure.  The framework-crate self-check (`.ckeletin/crate` must not contain
  CLI frameworks) remains hardcoded — it is a framework fact, true everywhere.

- **Hermetic consumer-layout fixture tests** (`.ckeletin/crate/tests/
  consumer_layout_fixtures.rs`): 11 in-process tests covering all matrix paths
  (custom layout + config pass/fail, absent config with/without scaffold,
  declared-but-missing layer, multi-crate domain, violation tests disabled).

### Consumer migration note (after updating past 0.2.21)

If you hand-patched `.ckeletin/crate/tests/arch_allowlist.rs` to extend
`DOMAIN_ALLOWED_DEPS` or added skip-guards — those changes are clobbered by
this update by design.  Move them to `ckeletin-project.toml` at your repo root:

```toml
# ckeletin-project.toml
[layers]
domain = ["crates/domain"]          # or your actual layer paths
infrastructure = ["crates/infrastructure"]
cli = ["crates/cli"]

[allowlists]
# Add each dep with a comment justifying it.
# Example (workhorse — 8 justified domain deps):
#   serde_json: JSON serialization for API responses
#   serde_yaml: YAML config deserialization
#   indexmap: ordered map for stable output
#   regex: pattern matching in domain rules
#   chrono: timestamp types in domain events
#   thiserror: typed error definitions
#   sha2: content hashing for deduplication
domain = ["serde", "serde_json", "serde_yaml", "indexmap", "regex", "chrono", "thiserror", "sha2"]
infrastructure = ["ckeletin"]
```

The file is yours — it lives outside `.ckeletin/` and survives every future
`just ckeletin-update`.

## [0.2.21] - 2026-06-10

### Fixed
- **`validate_level` is case-insensitive** — `INFO`, `Warn`, `OFF` are accepted
  again (the 0.2.19 strict validation rejected anything non-lowercase, breaking
  previously-valid configs). The value is lowercased before reaching `EnvFilter`.
- **`ckeletin-check-update` upstream-repo guard honors `json` format** — emits
  `{"applicable": false, "reason": "upstream repository"}` instead of prose, so
  the machine-readable contract holds in the upstream repo too.
- **Conform dangling-anchor gate now checks `path.rs::symbol` anchors** — the
  tokenizer previously dropped them (token didn't end in an extension), making
  the documented symbol check unreachable for evidence text.
- **Audit log directory tightened to 0700 unconditionally** — a pre-existing
  permissive log dir is now chmodded at init, not only freshly-created ones.
  Side effect: a user-owned read-only log dir is self-healed (owners may always
  chmod); dirs owned by another user (e.g. root after a sudo run) still fail
  with a clean typed error. Doc-comment honestly notes rotation-created files
  inherit the process umask until the next init.
- **`init.sh` license advice corrected** — LICENSE-MIT copyright is editable;
  LICENSE-APACHE must stay verbatim (its hash is pinned by `license_integrity`);
  copyright notices belong in NOTICE/source headers per the license's own terms.

## [0.2.20] - 2026-06-10

### Added
- **Dependency allowlist invariant tests** (`.ckeletin/crate/tests/arch_allowlist.rs`):
  domain `[dependencies]` == `{serde}`, infrastructure == `{ckeletin}`, and
  `.ckeletin/crate` must not contain CLI framework deps (clap etc.). Adding any
  new dep requires a conscious update to the allowlist constants. (Wave 2 / Finding #7)
- **Violation test drift guard** (`.ckeletin/crate/tests/violation_drift_guard.rs`):
  asserts every vendored `.ckeletin/tests/violations/*.rs` is byte-identical to its
  active project copy under `crates/*/tests/violations/`. Divergence fails CI loudly.
  Vendored = canonical (propagates to consumers); project = active (trybuild runs it).
  (Wave 2 / Finding #8)
- **`print_stdout = "deny"` + `print_stderr = "deny"` clippy lints** added to
  `[lints.clippy]` in the framework crate (`.ckeletin/crate/Cargo.toml`). Framework
  library code must not write to stdout/stderr; all output goes through the `Output`
  struct. Test files that emit skip signals via `eprintln!` carry per-file
  `#![allow(clippy::print_stderr)]` with justification. (Wave 2 / Finding #3)
- **CONFORMANCE.md header sync gate** in the conform generator: `just conform` now
  parses CONFORMANCE.md's stated spec version and requirement count and fails hard
  when they disagree with the mapping — so a spec bump that updates the mapping but
  not the prose is caught immediately. (Wave 2 / Finding #6b)
- **Bidirectional ENF-005 completeness check**: conform now also fails on mapping
  entries whose requirement ID is not in the spec (stale/invented IDs that inflate
  totals). (Wave 2 / Finding #9)
- **Dangling evidence anchor validation**: conform fails when any evidence string
  that looks like a file path (contains `/`, ends in `.rs`/`.toml` etc.) refers to
  a non-existent file, or when a `file.rs::symbol` anchor's symbol is absent from the
  file. (Wave 2 / Finding #1)
- **Check failure output surfaced**: failing conform checks now print the combined
  stdout+stderr of the failed command inline so failures are self-diagnosing without
  re-running manually. (Wave 2 / Finding #10)
- **JSON error objects for spec-load failures**: `--json` mode now emits a structured
  `{"status":"error","error":"..."}` object for spec-load and mapping-parse failures
  instead of plain human text. (Wave 2 / Finding #10)
- **Tag/version reconciliation gate** in `.github/workflows/release.yml`: asserts
  the pushed tag (`vX.Y.Z`) equals the cli package's `CARGO_PKG_VERSION` before
  releasing. Prevents the historical incident where `v0.2.0` was cut from package
  version `0.1.0`. (Wave 2 / Finding #12)
- **Full requirement metadata in vendored spec snapshot**: `conformance/requirements.json`
  now preserves `title`, `level`, `checkable`, `domain`, and `since` fields from the
  upstream spec (not just `id`). Refreshed from local spec repo at v0.8.0.
  (Wave 2 / Finding #11)

### Fixed
- **`date` command failure no longer panics** the conform generator. `current_date()`
  now degrades to `"unknown"` instead of `expect`-panicking. (Wave 2 / Finding #10)
- **OUT-005 enforcement level corrected**: mapping changed from `compile-time` to
  `linter` — the previous claim was false (domain CAN write to stdout without the
  clippy lints; the lint boundary is now real). (Wave 2 / Finding #3)
- **ARCH-006 enforcement level corrected**: mapping changed from `compile-time` to
  `design` — entry-point minimality is structural/review-enforced, not
  compiler-enforced. (Wave 2 / Finding #4)
- **ENF-005 evidence text corrected**: was "live, with vendored fallback" (describing
  ckeletin-go's behavior); now accurately describes the hermetic-by-default
  (`--refresh` for deliberate update) behavior. (Wave 2 / Finding #5)
- **OUT-004 stale anchor fixed**: removed citation of `audit_log_written_by_default`
  (test renamed); replaced with `audit_log_written_under_config_home_by_default`,
  `no_audit_flag_disables_the_log_file`, and `audit_log_content_contains_output_success_event_and_data`.
  (Wave 2 / Finding #2)
- **ARCH-003 anchor corrected**: `cli/Cargo.toml` → `crates/cli/Cargo.toml`.
  (Wave 2 / anchor cleanup)
- **CONFORMANCE.md updated** from spec v0.7.0/39 requirements to v0.8.0/40 requirements.
  Brittle prose counts (e.g. "main.rs is 102 lines") replaced with role descriptions.
  (Wave 2 / Finding #6a)

## [0.2.19] - 2026-06-10

### Fixed
- **Reachable panic on audit-log permission failure.** `prepare_file_appender`
  now uses `RollingFileAppender::builder().build()` (returns `Result`) instead
  of `tracing_appender::rolling::daily` (panics on file-creation failure).
  A chmod-555 directory previously caused exit 101 with a raw backtrace; it
  now flows to the clean error envelope + exit 1 path. (#finding-1)
- **Invalid log-level strings now produce a typed startup error.** Previously
  `build_filter` silently fell back to the default level, so
  `CKELETIN_LOG_FILE_LEVEL=info` (a plausible noise-reduction setting)
  silently emptied the audit stream by filtering out DEBUG shadow-log events.
  `validate_level` now rejects non-level strings at init time. "off" remains
  valid for JSON-mode stderr suppression. (#finding-3)
- **Empty `log_file_path` now errors explicitly.** An empty string previously
  scattered audit files outside the app directory; it now returns a clear
  `io::Error` instead of silently misbehaving. (#finding-7)
- **Shadow-log events emitted after the write, not before.** `Output::success`,
  `::message`, and `::error` previously emitted the audit tracing event before
  the write, so a failed write (e.g. broken pipe) produced a misleading
  `output.success` record for a run that never delivered output.
  Events are now emitted only after a successful write. (#finding-8)
- **Audit log directory created with mode 0700, files with 0600 (Unix).**
  Previously the directory and files were world-readable (0755/0644 via umask
  default). Every byte a downstream command renders lands in the audit file by
  design; narrowed permissions protect per-user audit contents. (#finding-5)
- **`process::run_capture` now captures child stderr.** Failures previously
  reported only "exited with status N"; child stderr diagnostics are now
  included in the error message. (#finding-11)

### Changed
- First-run audit notice now names the log directory and filename pattern
  (`app.log.<date>`) instead of a single file path that never exists (the
  daily roller always appends a date suffix). (#finding-6)

## [0.2.18] - 2026-06-10

### Fixed
- **`ckeletin-update` apply is now wholesale replacement** (HIGH). Replaced
  `git checkout <ref> -- .ckeletin/` with `git restore --source=<ref>
  --staged --worktree -- .ckeletin/`, which also deletes files absent from
  the source ref. Previously, files deleted upstream persisted forever in
  consumers. Regression test: `update_deletes_files_removed_upstream` in
  `.ckeletin/crate/tests/update_mechanism.rs`.
- **Rollback/restore no longer leaves upstream-added files staged** (HIGH).
  Both the tier-1 rollback in `ckeletin-update` and the `restore()` trap in
  `ckeletin-update-check-compatibility` now use `git restore --source=HEAD
  --staged --worktree -- .ckeletin/`. Previously `git checkout HEAD --
  .ckeletin/` left staged-but-uncommitted files behind, breaking the
  `rolled_back:true` machine verdict and the `just check` preflight for a
  subsequent update. Regression tests:
  `check_compatibility_leaves_no_staged_files` and
  `update_compile_fail_rolls_back_cleanly`.
- **Tag-pinned updates now work** (MEDIUM). `ckeletin-update version=<tag>`
  previously tried `git restore --source=ckeletin-upstream/<tag>` which is
  an invalid ref form for tags. Fix: fetch the tag explicitly with
  `git fetch ckeletin-upstream "<tag>"` then use `FETCH_HEAD`, mirroring
  `ckeletin-update-check-compatibility`'s existing approach. Regression test:
  `update_with_explicit_tag_works`.
- **`ckeletin-health` exits non-zero on a BROKEN workspace** (MEDIUM). The
  `|| echo "Workspace: BROKEN"` construct previously swallowed the exit code.
  `just check` now actually gates on a broken workspace. Regression tests:
  `health_exits_zero_on_clean_tree` and `health_exits_nonzero_on_broken_workspace`.
- **`ckeletin-check-update` has the upstream self-guard** (LOW). Consistent
  with `ckeletin-update`, `-dry-run`, and `-check-compatibility`.
- **`ckeletin-doctor` JSON now includes `components.rustfmt` and
  `components.clippy`** (LOW), with text/JSON parity verified by
  `doctor_json_and_text_report_same_components` in
  `.ckeletin/crate/tests/doctor.rs`.
- **`just init` now refuses on already-initialized repos** (MEDIUM). The
  guard checks for the upstream slug `peiman/ckeletin-rust` in `Cargo.toml`;
  if absent, `init` exits with a clear message. Override with `force=true`.
  Previously `just init` silently ran `rm -rf .git` on a consumer repo.
- **`just init` dirty-tree preflight now catches staged changes** (LOW).
  Added `--cached` to the `git diff` check so staged-but-not-committed
  changes are not silently discarded.
- **`just init` compile-check moved before `rm -rf .git`** (LOW). The
  verification now runs BEFORE destroying git history; a compile failure
  leaves the repo intact.
- **`{{name}}` quoted in root `init` recipe** (LOW). Prevents shell
  word-splitting for names that pass init.sh's alphanumeric validation.
- **Dead `binary_name` variable removed** from root `Justfile` (LOW). It
  was declared but never consumed; init.sh no longer rewrites it.
- **Stale comment in `init_smoke.rs` corrected** (LOW/INFO). The comment
  now correctly reflects that init.sh uses `--all-targets` (not just
  lib+bin).

### Added
- **Hermetic update-mechanism test suite** at
  `.ckeletin/crate/tests/update_mechanism.rs`: four fixture-based tests
  covering wholesale-replacement, rollback correctness, tag-pinned updates,
  and the CKELETIN_UPDATE_RESULT machine contract.
- **`ckeletin-health` exit-code tests** at
  `.ckeletin/crate/tests/health.rs`.
- **CI=true guard on tool-presence skips**: guard tests in
  `update_guard.rs` and all new test files now `panic!` (instead of
  silently returning) when `CI=true` and required tools are absent.
- **LICENSE reminder in `init.sh`** output: one-line notice to update the
  copyright holder/year in LICENSE files before distributing.

### Docs
- `docs/specs/2026-04-14-framework-update-mechanism.md`: status updated to
  "Shipped"; §Migration Flow and §Version Compatibility and Migrations marked
  **Deferred** with rationale (Principle 4 — Lean Iteration); §Testing
  Strategy updated to reflect the actual two-tier gate and the `git restore`
  rollback; `ckeletin-health` CI claim corrected.
- `docs/plans/2026-04-14-framework-update-mechanism.md`: status banner added
  ("IMPLEMENTED — historical record; do not execute").

## [0.2.17] - 2026-06-10

### Fixed
- **Framework crate version synced to `0.2.16`** — `Cargo.toml` had drifted 16
  patch releases behind `.ckeletin/VERSION`. A new `version_sync` integration
  test now asserts `CARGO_PKG_VERSION == trim(../VERSION)` at every `just check`,
  making future drift a build failure (Principle 9).
- **`Config::load()` rejects `--config` pointing at a directory** — previously a
  path that existed but was a directory silently fell through to defaults, defeating
  the explicit-config guard. Now returns a clear "is a directory" error.
- **Framework unit tests use `figment::Jail` for env-var isolation** — replaced all
  `std::env::set_var` / `remove_var` calls in `config.rs` with `figment::Jail`,
  providing serialised, race-free env+fs isolation per test and unblocking
  edition-2024 migration.

## [0.2.16] - 2026-06-05

### Added
- **Machine-readable command catalog (`catalog` command) — CKSPEC-AGENT-006.**
  The CLI now self-reports its own command surface as structured data: a
  `catalog` command emits, through the OUT-002 envelope, an enumeration of every
  command, subcommand, and flag — derived from the **same clap tree the parser
  uses** (`Cli::command()`), so it cannot drift from the actual command set.
  - `ckeletin::catalog` (framework crate, re-exported via `infrastructure`):
    the `Catalog` / `CatalogCommand` / `CatalogFlag` types — the cross-impl
    schema agreed with ckeletin-go (required core `long`/`required`/`takes_value`
    per flag, `global_flags` once at top level, recursive `commands`; optional
    `short`/`description`/`default`/`possible_values` where clap derives them).
    Defining the schema as a shared framework type means a derived project
    cannot emit a wrong-shaped catalog.
  - `crates/cli/src/catalog.rs` (worked example, like `ping`/`version`): the
    clap → `Catalog` walk (clap is cli-only by architecture).
  Closes the rust side of spec issue #9 (both agents endorsed SHOULD@v0.8.0).
  Spec text + conformance mapping land separately (the spec gets updated with a
  proven implementation, per the two-impl gate).

### Fixed
- **`conform` recipes no longer error in consumer repos.** `conform` /
  `conform-refresh` / `conform-report` validate ckeletin-rust against its own
  spec and are **upstream-only** — but the recipes propagate via `.ckeletin/`,
  and a consumer has no `conformance/requirements.json` (it's project-owned, only
  in the framework repo). So `just conform` failed with "cannot read vendored
  spec" on every downstream repo (found while updating the triz consumer
  v0.2.2→v0.2.14). The recipes now detect a consumer — the upstream `repository`
  slug is absent from the root `Cargo.toml` once `just init` rewrites it, the
  same signal the `ckeletin-update` self-guard uses — and **no-op with an
  explanation (exit 0)** instead of erroring. Regression test:
  `.ckeletin/crate/tests/conform_guard.rs`.

## [0.2.14] - 2026-06-05

### Added (agent-drivable: hardening for autonomous maintenance)
The framework's update/diagnostic surface now speaks machine, so an autonomous
orchestrator can drive it without human-shaped prose, prompts, or guesswork.
- **`ckeletin-check-update json`** — emits `{"current","latest","update_available"}`
  (or `{"error":"no_upstream_remote",…}`). The trigger signal for a maintenance
  loop: poll, branch on `update_available`.
- **`ckeletin-doctor json`** — emits a single object: `framework_version`,
  `toolchain` (pinned/msrv/rustc), and `tools` as booleans. Machine preflight
  for "is this environment ready". Regression test in `doctor.rs`.
- **`ckeletin-update` structured verdict** — prints a final
  `CKELETIN_UPDATE_RESULT={…}` line on every exit path with
  `status` (`updated` / `compile_failed` / `check_failed`), `from`, `to`,
  `committed`, `rolled_back` — so a driver can decide rollback / fix / escalate
  without parsing prose.
- **Non-interactive `just init`** — honours `CKELETIN_ASSUME_YES=1` to skip the
  uncommitted-changes prompt for agent/CI use. In a non-interactive shell
  WITHOUT that var it now refuses (exit 1) rather than blocking on a prompt or
  silently discarding work.

All text/human output is unchanged (these are additive `format` params / env
opt-ins), so existing usage is unaffected.

## [0.2.13] - 2026-06-04

### Fixed
- **`ckeletin-update` now guards with the real gate (`just check`), not just
  `cargo check`.** `cargo check` builds only lib/bins, so it did NOT run the
  clippy lint set or tests that `just check` enforces — meaning a release that
  tightened the gate could **auto-commit a red `just check`** on a consumer's
  branch with no signal at update time (reported by workhorse, 2026-06-04). Now:
  - a non-compiling update still rolls back fully (unchanged);
  - an update that compiles but fails `just check` is **left in the working tree,
    uncommitted** (not rolled back), with guidance to fix the new violations and
    commit — so you can fix forward instead of silently landing a red gate.
  - `ckeletin-update-check-compatibility` likewise runs `just check` now, so it
    surfaces tightened-gate failures *before* you update.

### Changed
- **`float_cmp` is now scoped to library/binary code, not tests.** The hardened
  clippy gate (0.2.11) ran `float_cmp` over `--all-targets`, flagging idiomatic
  exact-sentinel test assertions like `assert_eq!(score, 0.0)` (workhorse hit
  ~18 such sites). It is now a separate `--lib --bins` pass, keeping the safety
  for real logic without fighting correct test assertions. The other hardened
  lints (cast safety, etc.) remain on all targets — they caught a real
  truncation bug downstream.

### Upgrading from 0.2.11 (note for adopters)
The 0.2.11 hardened clippy gate may flag pre-existing cast sites in your code on
the first `just check` after updating. Fix forward: use `try_from` (returning an
error) where a value can genuinely overflow or lose sign; add a reasoned
`#[allow(clippy::cast_…)]` with a one-line rationale where the conversion is safe
by construction (e.g. a bounded counter). Run `just ckeletin-update-check-compatibility`
first to preview what the new gate will flag.

## [0.2.12] - 2026-06-04

### Added
- **Fuzzing worked example with bolero.** Chose
  [bolero](https://github.com/camshaft/bolero) because it is the only mainstream
  Rust fuzzer whose targets run on **stable** (the scaffold's pinned toolchain) —
  unlike cargo-fuzz, which needs nightly.
  - `crates/domain/tests/fuzz_ping.rs` — the `ping` worked example's fuzz
    counterpart: feeds arbitrary messages into `PingResult` and asserts `Display`
    and serde round-trip never panic for any input. It runs as an ordinary
    `cargo test` (bounded, deterministic, generative) on stable, so it is a
    **regression guard inside `just check`** — every PR gets fuzz-generated
    coverage with no nightly and no extra tooling.
  - `ckeletin-fuzz` recipe — exercises the bolero targets on stable
    (`cargo test --test fuzz_ping`), for iterating on a target directly.
  - `bolero` added as a **dev-dependency** (does not relax domain's runtime
    "only serde" boundary); a `[profile.fuzz]` is defined for bolero.
  - `ckeletin-doctor` reports cargo-bolero.
  - **Deliberately not wired:** coverage-guided *active* fuzzing (bolero's
    libfuzzer engine) needs nightly AND a dedicated fuzz crate excluded from the
    workspace — its sancov instrumentation otherwise leaks into sibling test
    binaries (the cli integration tests) and fails to link on both macOS arm64
    and Linux. Documented in the `ckeletin-fuzz` recipe as the next step for
    teams that want continuous coverage-guided fuzzing.

## [0.2.11] - 2026-06-04

### Added
- **Static analysis hardening (SAST).** Research showed dedicated SAST adds
  little marginal value in Rust beyond clippy + cargo-deny (memory-safety
  removes the bug classes tools like semgrep target; cargo-audit duplicates
  cargo-deny's advisory DB). So this takes the two genuinely additive steps:
  - **Hardened `ckeletin-clippy`** — denies a curated set of security/correctness
    lints on top of `-D warnings`: numeric-cast safety
    (`cast_possible_truncation`, `cast_sign_loss`, `cast_possible_wrap`,
    `cast_precision_loss`), float pitfalls (`float_cmp`, `lossy_float_literal`),
    and footguns (`dbg_macro`, `todo`, `unimplemented`, `mem_forget`,
    `rc_buffer`, `verbose_file_reads`, `wildcard_dependencies`). Gates via
    `just check` and (SSOT) the lefthook pre-commit clippy hook now calls
    `just ckeletin-clippy` so both enforce the identical set.
  - **`ckeletin-geiger` recipe** — reports the `unsafe` surface across the
    dependency tree with [cargo-geiger](https://github.com/geiger-rs/cargo-geiger)
    (`--forbid-only`). ADVISORY ONLY — an unsafe count is a metric, not a gate,
    so it never blocks `just check`.
  - `ckeletin-doctor` reports cargo-geiger presence.
  Deliberately did NOT add cargo-audit (redundant with cargo-deny) or semgrep
  (thin Rust ruleset, low marginal value).

## [0.2.10] - 2026-06-04

### Added
- **SBOM generation + vulnerability scanning (supply-chain readiness).**
  - `ckeletin-sbom` recipe — generates `sbom.cdx.json`, a CycloneDX 1.5 SBOM of
    the CLI binary's full dependency graph, using
    [cargo-cyclonedx](https://github.com/CycloneDX/cyclonedx-rust-cargo) (the
    official OWASP CycloneDX cargo plugin; stable toolchain, no nightly).
  - `ckeletin-sbom-scan` recipe — generates then scans the SBOM with
    [grype](https://github.com/anchore/grype), failing on High severity or above.
  - Both standalone (external tools, not in `just check`). `ckeletin-doctor`
    reports cargo-cyclonedx + grype presence. Generated `*.cdx.json` are
    gitignored.
  - Worked example (project-owned): a `sbom` CI job that generates + scans and
    uploads the SBOM as a build artifact for compliance/consumers.
  Chose the Rust-native OWASP generator over syft for a leaner footprint (one
  external binary) while keeping grype for parity with ckeletin-go's scanner.

## [0.2.9] - 2026-06-04

### Added
- **Secret scanning with gitleaks (CKSPEC-ENF-001).** Detects hardcoded
  credentials committed to the repo, using the industry-standard
  [gitleaks](https://github.com/gitleaks/gitleaks) (MIT, single static binary).
  - `ckeletin-secrets` recipe — scans the working tree. Standalone, not part of
    `just check` (gitleaks is an external non-cargo tool, so a missing gitleaks
    never blocks the cargo gate).
  - `.ckeletin/configs/gitleaks.toml` — framework default config (extends the
    built-in ruleset, excludes `target/`); override via a root `.gitleaks.toml`.
  - `ckeletin-doctor` now reports gitleaks presence.
  - Worked examples (project-owned, kept/replaced by adopters): a lefthook
    pre-commit staged scan that skips cleanly when gitleaks is absent but fails
    on a real secret, and a `secret-scan` CI job that scans full git history via
    the gitleaks **CLI** (not the commercial gitleaks-action).
  Mirrors ckeletin-go's secret scanning.

## [0.2.8] - 2026-06-04

### Added
- **`ckeletin-doctor` recipe.** Reports the development environment — framework
  version, pinned toolchain + MSRV (read from `rust-toolchain.toml` / `Cargo.toml`,
  so it stays SSOT) and installed `rustc`, plus presence of the tools the
  framework depends on (`cargo-deny`, `cargo-llvm-cov`, optional `cargo-nextest`,
  `just`, and the rustfmt/clippy components). Informational only — always exits 0,
  so it is intentionally not part of `just check`. Mirrors ckeletin-go's
  `task doctor`. Smoke test: `.ckeletin/crate/tests/doctor.rs`.
- **`ckeletin-version` recipe.** Prints the framework version (parity with
  ckeletin-go's `task version`).

### Notes
- Remaining ckeletin-go tasks are deliberately not ported. The `validate:*`
  ADR-enforcement suite is already achieved at compile time (trybuild violation
  tests + `framework_purity`) and by `conform`; the `check:*`/`test:*`/`build:*`
  variants collapse into the single `check` gateway and standard cargo; and
  GoReleaser/`generate:config:*`/`tidy` are Go-toolchain specific. Heavier
  capabilities (secret scanning, SAST, SBOM, fuzzing, benchmarks, `setup`) remain
  open decisions rather than silent external-tool dependencies.

## [0.2.7] - 2026-06-04

### Added
- **`ckeletin-update-check-compatibility` recipe.** Applies the upstream
  `.ckeletin/` to the working tree, runs `cargo check --workspace`, then
  restores the committed framework via a trap (interrupt-safe) — letting an
  adopter confirm an update compiles against their code without keeping it.
  Brings the Rust framework to parity with ckeletin-go's
  `task ckeletin:update:check-compatibility`. No import rewriting is needed
  (Rust references crates by name, not an embedded module path).
- **Upstream self-guard on the update recipes.** `ckeletin-update`,
  `ckeletin-update-dry-run`, and `ckeletin-update-check-compatibility` now
  short-circuit (exit 0 with a message) when run inside the ckeletin-rust
  upstream repo itself, detected via the workspace `repository` slug in the
  root `Cargo.toml` (`just init` rewrites it for derived projects). Mirrors
  ckeletin-go's go.mod module-path guard. Regression test:
  `.ckeletin/crate/tests/update_guard.rs`.

### Changed
- The upstream remote URL and identity slug are now SSOT `just` variables
  (`ckeletin_upstream_url`, `ckeletin_upstream_slug`) instead of being
  hardcoded across the update recipes.

## [0.2.6] - 2026-06-04

### Added
- **Anchored conformance evidence (CKSPEC-ENF-008).** `just conform` now
  exits non-zero on any `met` requirement that has no automated check, no
  violation test, and no written `violation_evidence` — an unbacked "met"
  can no longer pass the gate or reach the published report. The gate
  (`lacks_anchor`) runs after the completeness check; unit tests
  `anchored_met_passes` / `unanchored_met_is_rejected` prove it.
- **Published machine-readable conformance report (CKSPEC-ENF-010).** The
  generator projects `conformance-mapping.toml` into a deterministic
  `conformance-report.json` at the repo root — sorted requirement keys,
  alphabetical fields, **no timestamp** — so it is byte-stable and a spec
  repo can aggregate it instead of hand-authoring (the aggregator stamps
  the fetch date). `just conform` regenerates it in memory and **fails on
  drift** (sync-check); `just conform-report` rewrites it. Schema mirrors
  ckeletin-go's report (`implementation`, `requirements`, `spec_version`,
  `summary`). Unit tests `report_projection_is_deterministic` /
  `sync_check_detects_drift`.
- `conform-report` recipe in `.ckeletin/Justfile` — regenerate the
  published report after editing the mapping.

### Notes
- CKSPEC-ENF-009 (conformance gate on release) is wired at the project
  level, not the framework level: a tag-triggered `release.yml` gates its
  publish job on the `conform` job, and a scheduled `spec-drift.yml`
  watches the live upstream spec. These ship as worked examples adopters
  keep or replace, like `ci.yml`.

## [0.2.5] - 2026-06-03

### Added
- **Build identity (`build_info::BuildInfo`).** A prefix-agnostic framework
  primitive that surfaces the git provenance baked into a binary at compile
  time — version + commit + date + dirty — rendered by `version_line()`
  (mirrors ckeletin-go's `--version`: `"<version>, commit <commit>, built
  <date> (dirty)"`). The scaffold ships the worked example of consuming it:
  `crates/cli/build.rs` bakes the identity (one atomic `git describe --dirty`,
  so there is no false-clean gap; degrades to `unknown` on any git failure) and
  a `version` command renders it in human + JSON, with `--version` wired to the
  same formatter. Build-identity surfacing only; runtime staleness checking is
  left to the adopter (out of the shared cross-language contract). First
  consumer: workhorse (SH-004). Implements CKSPEC-OUT-006.

## [0.2.4] - 2026-05-31

### Changed
- The audit log (CKSPEC-OUT-004) now defaults to a stable per-user location
  instead of `./logs/` relative to the working directory. A relative
  `log_file_path` is anchored under `~/.config/<app>/` by default (XDG-style,
  uniform on every platform including macOS). New `log_location` config field:
  `"config"` (default) or `"platform"` (the OS-native app-data dir, e.g.
  `~/Library/Application Support/<app>` on macOS). An absolute `log_file_path`
  still overrides entirely. Resolution is dependency-free (env vars only — no
  new crates, no copyleft). The first-run notice prints the resolved path.

## [0.2.3] - 2026-05-29

### Changed
- Audit logging (CKSPEC-OUT-004) is now **on by default**
  (`Config.log_file_enabled` defaults to `true`), and
  `Output::success`/`message`/`error` shadow-log the *rendered data*, not
  just the command name — so the audit stream contains what the user saw.
  Downstream projects receive this on `just ckeletin-update` and will start
  writing `logs/app.log` by default; opt out with `log_file_enabled = false`
  (or the `--no-audit` flag if the consumer wires it into its CLI).

### Fixed
- `just init <name>` produced a non-compiling, un-committed project.
  The strip-demo step deleted `ping` (the only subcommand), leaving an
  empty `Commands` enum the entry point could not match exhaustively,
  and a `sed '/ping/Id'` line delete mangled the integration-test file
  into invalid Rust. init now keeps `ping` as the renamed worked
  example (as the ckeletin-go scaffold does) and verifies with
  `cargo check --all-targets`. The `init_smoke` test now builds and
  tests the initialized project, and CI gates it (upstream-only).
  Fixes #1.

### Security
- Bumped `rustls-webpki` to 0.103.13 (RUSTSEC-2026-0104: reachable
  panic parsing certificate revocation lists).

## [0.2.2] - 2026-04-22

### Added
- `Output::message(command, msg, writer)` — emit a human-addressed
  success response with no structured data. Human mode writes the
  message with a trailing newline; JSON mode wraps it in an
  envelope with `data: {"message": msg}` (structured, not a raw
  string blob in the data slot). Replaces the common wart of
  passing `&format!("...")` to `Output::success` for "no data to
  report" success paths.

### Spec alignment
- Neither CKSPEC-OUT-003 nor CKSPEC-OUT-005 forbade the prior
  pattern — it produced structurally valid envelopes — but the
  structure was inconsistent. `Output::message` formalizes the
  no-data-success shape so downstream consumers can rely on
  `data.message` always being a string.

## [0.2.0] - 2026-04-14

### Added
- Extracted framework library into `.ckeletin/crate/`
- Output, config, logging, process modules from infrastructure
- Framework update mechanism (`just ckeletin-update`)
- Init flow (`just init name=<name>`)
- Violation test templates in `.ckeletin/tests/violations/`
- Two-level Justfile: framework tasks in `.ckeletin/Justfile`
