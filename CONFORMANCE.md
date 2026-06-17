# Ckeletin Spec v0.8.0 — Rust Conformance Report

**Implementation:** ckeletin-rust
**Spec version:** 0.8.0
**Report date:** 2026-06-10
**Total:** 40 requirements — 40 met

This report is reconciled with `conformance-mapping.toml` (the machine-readable
source of truth) and is validated by `just conform` (`.ckeletin/conform/`),
which runs in CI. When prose and mapping disagree, the mapping wins and this
file is corrected to match.

`just conform` now enforces that this header's stated spec version and
requirement count match the mapping — so the PR-#26 incident (mapping bumped
to 0.8.0, prose left at 0.7.0) cannot recur silently.

Per Principle 10, this is a conformance report from a second implementation —
a retrospective, not an audit. Cross-implementation feedback with ckeletin-go
continues to refine the spec.

> **Changed since the 2026-06-04 report (spec v0.7.0/39 requirements):**
> spec advanced to v0.8.0, adding **CKSPEC-AGENT-006** (machine-readable command
> catalog — *met*). Conformance wave 2 also corrected several enforcement claims:
> OUT-005 raised from `compile-time` to `linter` (clippy `print_stdout/print_stderr`
> deny added to domain/infrastructure/.ckeletin/crate Cargo.toml); ARCH-006
> corrected from `compile-time` to `design` (entry-point minimality is structural,
> not compiler-enforced); ARCH-004 notes added clarifying intra-crate module
> boundary limits. New gates added: dangling evidence anchor detection, bidirectional
> ENF-005 completeness (extra mapping entries also fail), CONFORMANCE.md header
> sync check, dependency allowlist invariant tests, and violation-test drift guard.

---

## Architecture (7/7 met)

| ID | Title | Status | Enforcement | Violation Test / Evidence |
|----|-------|--------|-------------|----------------|
| CKSPEC-ARCH-001 | Four-layer architecture | met | compile-time | Workspace structure; `crates/domain/tests/violations/domain_imports_infrastructure.rs` |
| CKSPEC-ARCH-002 | Directed dependencies | met | compile-time | `domain_imports_infrastructure.rs`, `infra_imports_domain.rs` |
| CKSPEC-ARCH-003 | CLI framework isolation | met | compile-time | `domain_imports_clap.rs`, `infra_imports_clap.rs` |
| CKSPEC-ARCH-004 | Business logic isolation | met | compile-time | `domain_imports_figment.rs`, `domain_imports_tracing.rs` (see note) |
| CKSPEC-ARCH-005 | Infrastructure independence | met | compile-time | `infra_imports_domain.rs` |
| CKSPEC-ARCH-006 | Entry point minimality | met | design | `crates/cli/src/main.rs` — bootstrap only (see note) |
| CKSPEC-ARCH-007 | Package location enforcement | met | design | Structural (see note) |

**Evidence:** `crates/domain/Cargo.toml` lists only `serde`. `crates/infrastructure/Cargo.toml` has no domain or cli dependency. Writing `use clap::Parser` in domain → compiler error E0432. Seven trybuild compile-fail tests (five domain + two infra) verify the boundaries on every `cargo test`. Dependency allowlist invariant tests in `.ckeletin/crate/tests/arch_allowlist.rs` make adding a forbidden dep a loud CI failure.

**ARCH-004 note:** the compile-time claim applies to the cross-crate boundary — domain cannot import infrastructure/cli, enforced by Cargo. Intra-crate module isolation within the single `domain` crate is enforced at design level: Rust cannot cheaply compile-time-enforce intra-crate module boundaries. The multi-crate split is the documented upgrade path when domain grows. Routed to spec feedback (Wave 4) for requirement calibration.

**ARCH-006 note (corrected):** `main.rs` is the bootstrap entry — argument parsing, config loading, logging init, and dispatch. Feature logic lives in domain; the entry contains no business logic. Enforcement is `design`, not `compile-time` — the compiler enforces that only cli has a bin target, but cannot enforce that `main.rs` stays minimal; that is a code-review and coverage judgment.

**ARCH-007 note:** file placement is enforced structurally by the Cargo workspace layout — a stray `.rs` file at the workspace root belongs to no crate and is not built — not by a dedicated file-placement linter. Enforcement level is therefore `design`, not `compile-time`.

---

## Enforcement (10/10 met)

| ID | Title | Status | Evidence |
|----|-------|--------|----------|
| CKSPEC-ENF-001 | Automated enforcement required | met | lefthook pre-commit (fmt, clippy), cargo-deny, CI via `just check` |
| CKSPEC-ENF-002 | Enforcement ladder | met | compile-time (architecture), linter (OUT-005), CI (coverage, conform, init-smoke) |
| CKSPEC-ENF-003 | Document enforcement gaps | met | This report + the audit table below document all gaps |
| CKSPEC-ENF-004 | Enforcement audit table | met | See audit table below; reconciled with `conformance-mapping.toml` |
| CKSPEC-ENF-005 | Conformance mapping completeness | met | `just conform` is hermetic by default; checks both directions (spec IDs not in mapping, AND mapping IDs not in spec) |
| CKSPEC-ENF-006 | Violation tests for enforcement claims | met | 7 trybuild violation tests; `just conform` verifies each declared violation-test file exists and flags unproven above-honor claims |
| CKSPEC-ENF-007 | Automatic feedback signals | met | `just conform` emits feedback signals in its report summary (`feedback_signals`) |
| CKSPEC-ENF-008 | Anchored conformance evidence | met | `just conform` **exits non-zero** on any `met` requirement with no check, violation test, or written `violation_evidence`; unit tests `anchored_met_passes` / `unanchored_met_is_rejected` |
| CKSPEC-ENF-009 | Conformance gate on release | met | `.github/workflows/release.yml` gates `publish` on the `conform` job (`needs:`); scheduled `spec-drift.yml` watches the live spec |
| CKSPEC-ENF-010 | Published machine-readable conformance report | met | Deterministic `conformance-report.json` projected from the mapping; `just conform` sync-checks it (fails on drift); unit tests `report_projection_is_deterministic` / `sync_check_detects_drift` |

**Generator:** `just conform` runs the `ckeletin-conform` crate (`.ckeletin/conform/`). It loads the committed `conformance/requirements.json` snapshot (hermetic — no network; `--refresh` re-fetches from the spec repo), checks both completeness directions (ENF-005), enforces the anchoring gate (ENF-008), validates dangling evidence anchors (file paths and test symbols), sync-checks the published report (ENF-010), checks CONFORMANCE.md header version/count, runs each mapped check with failure output surfaced, verifies declared violation-test files exist (ENF-006), and emits feedback signals (ENF-007). Gated by the CI `conform` job.

**ENF-005 (bidirectional, corrected):** the generator now checks BOTH directions: spec IDs absent from the mapping (unmapped requirements) AND mapping IDs not in the spec (stale/invented IDs that inflate totals). Both directions fail the gate. Evidence text corrected from "live, with vendored fallback" to the accurate "hermetic by default, `--refresh` for deliberate update."

**ENF-006 proof:** every above-honor-system claim now carries proof, so `just conform` reports **0 feedback signals**.

**ENF-008 anchoring:** every `met` requirement must be anchored to verifiable evidence — at least one of an automated check, a violation test, or written `violation_evidence`. The generator refuses to publish a `met` that has no backing.

**ENF-009 release gate (updated):** `release.yml` is tag-triggered (`v*`); its `publish` job declares `needs: conform`. A new step asserts the pushed tag (`vX.Y.Z`) equals the cli package's `CARGO_PKG_VERSION` — preventing the historical incident where `v0.2.0` was cut from package version `0.1.0`. Tag-namespace note: future ckeletin framework pins may use a `ckeletin-v*` prefix to distinguish from binary release tags.

**ENF-010 published report:** `conformance-report.json` at the repo root is a deterministic projection of `conformance-mapping.toml` — sorted keys, alphabetical fields, no timestamp.

---

## Testing (4/4 met)

| ID | Title | Status | Evidence |
|----|-------|--------|----------|
| CKSPEC-TEST-001 | Test-driven development | met | Honor system; git history shows test+impl atomicity |
| CKSPEC-TEST-002 | Minimum coverage threshold | met | `just coverage` enforces 85% (cargo-llvm-cov), **gated by the CI coverage job**. Workspace is ~99.8%; the build-time `.ckeletin/conform` generator is a documented exclusion |
| CKSPEC-TEST-003 | Dependency injection over mocking | met | Writer injection in `Output`; zero mock crates in `Cargo.lock` |
| CKSPEC-TEST-004 | Atomic test commits | met | Honor system; git history |

---

## Output (6/6 met)

| ID | Title | Status | Enforcement | Evidence |
|----|-------|--------|-------------|----------|
| CKSPEC-OUT-001 | Three-stream output separation | met | script | stdout (data), stderr (status via tracing), file (audit) |
| CKSPEC-OUT-002 | Machine-readable output mode | met | script | `--output json`; `crates/cli/tests/cli.rs` verifies the JSON envelope on stdout |
| CKSPEC-OUT-003 | Standardized output envelope | met | script | `Envelope { status, command, data, error }`; `output::tests::envelope_*` |
| CKSPEC-OUT-004 | Shadow logging | met | script | Shadow-logs rendered data; audit on by default (`--no-audit` opts out) |
| CKSPEC-OUT-005 | Output isolation from business logic | met | linter | `print_stdout = "deny"` + `print_stderr = "deny"` in domain/infra/.ckeletin/crate `[lints.clippy]` (see note) |
| CKSPEC-OUT-006 | Build identity in version output | met | script | `version` + `--version` surface version/commit/date/dirty; each degrades to `"unknown"`; `version_command_json_has_fields` in `crates/cli/tests/cli.rs` |

**OUT-004:** every `Output` method shadow-logs the rendered data to the audit layer. File audit logging is on by default, active in both human and JSON modes. Tests: `audit_log_written_under_config_home_by_default`, `no_audit_flag_disables_the_log_file`, and `audit_log_content_contains_output_success_event_and_data` in `crates/cli/tests/cli.rs`.

**OUT-005 note (corrected from compile-time to linter):** domain and infrastructure previously had no lint preventing `println!` — only the absence of `std::io` imports via the Cargo dependency boundary. Wave 2 added `[lints.clippy] print_stdout = "deny" / print_stderr = "deny"` to `crates/domain/Cargo.toml`, `crates/infrastructure/Cargo.toml`, and `.ckeletin/crate/Cargo.toml`. This fires at `cargo clippy` time (the `linter` enforcement rung), not just at link time. The cli crate is exempt as the designated presentation layer. Verified: adding `println!("x")` to domain produces `error: use of println!` and fails `just ckeletin-clippy`.

---

## Agent Readiness (6/6 met)

| ID | Title | Status | Evidence |
|----|-------|--------|----------|
| CKSPEC-AGENT-001 | Universal agent guide | met | `AGENTS.md` |
| CKSPEC-AGENT-002 | No provider-specific content in universal guide | met | `AGENTS.md` is provider-neutral |
| CKSPEC-AGENT-003 | Provider-specific guides follow provider guidance | met | `CLAUDE.md` references `AGENTS.md` |
| CKSPEC-AGENT-004 | Agent guide completeness | met | Covers purpose, architecture, commands, conventions, testing, troubleshooting |
| CKSPEC-AGENT-005 | CLI as the agent interface | met | `--output json` machine-readable mode; no protocol layer required |
| CKSPEC-AGENT-006 | Machine-readable command catalog | met | `catalog` command emits the cross-impl schema via the OUT-002 envelope, derived from the clap command tree |

---

## Changelog (7/7 met)

| ID | Title | Status |
|----|-------|--------|
| CKSPEC-CL-001 | CHANGELOG.md in repository root | met |
| CKSPEC-CL-002 | Keep a Changelog format | met |
| CKSPEC-CL-003 | ISO 8601 dates | met |
| CKSPEC-CL-004 | Semantic Versioning | met |
| CKSPEC-CL-005 | Unreleased section | met |
| CKSPEC-CL-006 | Human-curated, not auto-generated | met |
| CKSPEC-CL-007 | Version comparison links | met |

---

## Enforcement Audit Table (CKSPEC-ENF-004)

| Decision | Mechanism | Level | Status | Violation Test | Gap |
|----------|-----------|-------|--------|----------------|-----|
| Four-layer architecture | Cargo workspace boundaries | compile-time | Full | 7 trybuild tests | — |
| Directed dependencies | Cargo.toml dependency graph | compile-time | Full | trybuild tests | — |
| CLI framework isolation | domain/infra Cargo.toml exclude clap | compile-time | Full | `domain_imports_clap.rs` | — |
| Business logic isolation | domain Cargo.toml excludes infra deps | compile-time | Full | 4 trybuild tests | intra-crate module isolation is design-level only |
| Infrastructure independence | infra Cargo.toml excludes domain/cli | compile-time | Full | 2 trybuild tests | — |
| Dependency boundary allowlist | arch_allowlist.rs invariant tests | script | Full | tests assert exact dep sets | Adding a dep requires updating the allowlist |
| Output isolation (domain/infra) | clippy print_stdout/print_stderr = deny | linter | Full | probe verified (println! in domain → clippy error) | cli crate is exempt (presentation layer) |
| Package location | workspace layout (structural) | design | Structural | — | No file-placement linter |
| Entry-point minimality | bootstrap-only `main.rs` | design | Structural | — | Code-review / coverage judgment, not compiler-enforced |
| Code formatting | cargo fmt + lefthook | pre-commit | Full | — | No violation test |
| Lint standards | clippy -D warnings | pre-commit | Full | — | No violation test |
| License + advisory scanning | cargo-deny | pre-commit + CI | Full | — | No violation test |
| Coverage threshold | cargo-llvm-cov 85% | CI | Full | — | conform generator excluded (documented) |
| Conformance completeness (fwd) | `just conform` fail-on-unmapped | CI (script) | Full | `find_unmapped_*` in conform | — |
| Conformance completeness (rev) | `just conform` fail-on-extra | CI (script) | Full | `find_extra_*` in conform | — |
| Dangling evidence anchors | `just conform` file/symbol check | CI (script) | Full | `dangling_anchor_*` in conform | — |
| CONFORMANCE.md header sync | `just conform` version+count check | CI (script) | Full | `conformance_md_header_parse_*` in conform | — |
| Conformance violation proof | `just conform` checks test files | CI (script) | Full | `lacks_proof_*` in conform | All claims carry tests or CI-gated evidence (0 feedback signals) |
| Anchored evidence | `just conform` fail-on-unanchored-met | CI (script) | Full | `anchored_met_passes`, `unanchored_met_is_rejected` | — |
| Conformance gate on release | `release.yml` publish `needs: conform` | CI | Full | — | — |
| Tag/version reconciliation | `release.yml` tag == CARGO_PKG_VERSION | CI | Full | — | — |
| Spec freshness vs. latest | scheduled `spec-drift.yml` opens an issue | CI (scheduled) | Full | — | Detects drift; reconciling it is a human action |
| Published report determinism | `just conform` report sync-check | CI (script) | Full | `report_projection_is_deterministic`, `sync_check_detects_drift` | — |
| Violation test drift guard | `violation_drift_guard.rs` byte-identity | script | Full | test asserts byte-identity | Adding a new violation file requires updating both copies |
| Build identity in version | `build.rs` bakes commit/date; honest "unknown" | script | Full | `version_command_json_has_fields` | — |
| Shadow logging | tracing events (data) + default-on audit | script | Full | output.rs + cli.rs audit tests | — |
| TDD / atomic commits / changelog curation | AGENTS.md + CLAUDE.md | honor system | — | N/A | Cannot automate intent |
| Conventional commits | lefthook commit-msg | pre-commit | Full | — | No violation test |
| Scaffold init flow | `init_smoke` test | CI (upstream-only) | Full | `init_smoke` | — |

---

## Cross-Implementation Observations (Principle 10)

1. **Compile-time enforcement of architecture is real in Rust.** Cargo workspace crate boundaries make the compiler the linter; Go needs go-arch-lint. Both satisfy the requirements; `enforcement_level` makes the difference visible.

2. **Conformance reporting rots faster than code.** This report drifted from the code (the generator existed but the prose said it didn't; the spec advanced from v0.3.0 through v0.8.0). The fix is structural: `conformance-mapping.toml` is the SSOT, `just conform` validates it, CI gates it, and the new CONFORMANCE.md header sync check means a spec bump that updates the mapping without updating the prose fails the gate immediately — prose can no longer silently diverge.

3. **Honest partials beat false "met"s — then close them.** OUT-004 shadow logging was first reported `partial` (rather than claimed met-with-a-hedge), making the gap visible; it was then implemented properly and is now genuinely met. Truth-Seeking (Principle 1): surface the gap, don't bury it in a "when enabled" qualifier, then fix it.

4. **A scaffold's headline flow must be gated.** `just init` shipped broken (issue #1) because its guard test was `#[ignore]`d and never run in CI. The lesson for the spec: enforcement claims include the *scaffold's own* tooling, not just the generated project.

5. **Hermetic conform + a drift watcher beats live-fetch — a deliberate divergence.** ckeletin-go's `conform` live-fetches the spec, so its gate verifies against the latest spec but breaks when the network or the spec repo is down. ckeletin-rust instead gates against a *committed* `conformance/requirements.json` snapshot (reproducible, offline, deterministic) and pairs it with a scheduled `spec-drift.yml` that watches the live spec and files an issue when it advances. Both are defensible; the divergence is recorded here.

6. **A green report is only worth its anchors (ENF-008).** A conformance report is a trust artifact; "met" is worthless if it can be asserted without backing. The anchoring gate makes the generator refuse to publish a `met` that has no check, no violation test, and no written evidence — so the cost of a false "met" is a red build, not a silent lie (Principle 1 mechanized).

7. **Evidence anchors must be live, not cited-and-forgotten.** Wave 2 found a stale anchor: OUT-004 cited `audit_log_written_by_default`, a test that no longer exists (it was renamed). The dangling anchor gate now catches this class automatically: every file-path anchor in the mapping must exist on disk, and `file.rs::fn_name` anchors must have the named symbol present in the file.
