# ckeletin-rust — Full Code Review

**Date:** 2026-06-09 · **Method:** 9 parallel dimension reviewers → dedup → adversarial verification (3 lenses for high/critical, skeptic for the rest) → completeness critic. 119 agents total.
**Baseline:** `just check` green on clean tree (framework v0.2.16, spec v0.8.0). ~3,900 LOC Rust, 82 tracked files.
**Judged against:** the ckeletin spec (~/dev/ckeletin/spec/*.yaml v0.8.0), the 10-principle manifesto (~/dev/manifesto), and the repo's own documented contracts.

**Result: 0 critical · 7 high · 29 medium · 29 low · 18 info — 83 confirmed findings; 5 candidate findings refuted in adversarial verification.**

---

## Dimension summaries

### correctness

The core data flow (CLI args -> config -> domain -> Output -> envelope -> shadow audit log) is solid and well-tested: flag-activated JSON mode is correct in both success and error paths (envelope names the failing subcommand, nothing leaks to stderr), the build-identity pipeline was verified empirically to self-correct in both clean->dirty and dirty->clean directions in a throwaway repo, malformed TOML/env values produce clear errors with exit 1, missing-HOME degrades exactly as documented, and the catalog derivation from the live clap tree emits clean JSON (switches correctly suppress bogus possible_values). The defects cluster in the seams the integration tests don't cover: output-mode resolution is computed twice from different inputs, producing a mixed-mode binary when JSON is activated via config/env (success = JSON envelope, error = human stderr — a broken CKSPEC-OUT-002 MUST) and making `--output text` unable to override config json despite the documented flag-highest precedence; and the vendored framework's logging init has a reachable panic (exit 101) when the audit file can't be created in an existing directory — a real-world trap (one sudo run bricks every later invocation) in code that propagates to all downstream repos. Two MUST-adjacent audit-log gaps (a config knob that silently empties the audit stream; a first-run notice naming a file that never exists) plus three low path-handling edges round it out. No defects found in domain logic, the envelope serde shapes, exit-code semantics on the tested paths, or the conform gate logic itself.

### architecture

The compiled core of this architecture is genuinely strong: domain's runtime dependency tree is exactly serde (cargo tree verified), the seven trybuild violation tests pass and are airtight for what they enumerate — dev-dependencies fold into the generated test crate so even a dev-dep leak trips them, pinned .stderr snapshots (E0432/E0433 unresolved-import) prevent wrong-reason passes, and domain_imports_ckeletin.rs closes the framework re-export bypass; the vendored framework crate itself is cleanly layered (no clap, writer-injection throughout, zero direct stream writes, purity tests against project-specific leakage), handlers are exemplary thin pass-throughs, and the catalog's derive-from-the-same-clap-tree design makes drift structurally impossible. The weaknesses are not in the fences but in the claims about the fences: the published conformance report asserts compile-time guarantees that don't exist (OUT-005's 'domain cannot write to stdout' is demonstrably false — println! compiles fine and no clippy print lint is configured; ARCH-006's 'compile-time' contradicts the repo's own audit table which already corrected it to 'design'), the Entry layer violates ARCH-003's letter by importing clap directly in main.rs, and the enforcement net is an enumerated denylist — a novel forbidden dep in domain, clap added to the propagated .ckeletin/crate, or cross-feature imports inside the single domain crate would all sail through green. The enforcement ladder (Principle 9) is used at the top rung where it was easy, but two feasible rungs sit unused: per-crate clippy print lints and allowlist invariant tests over the Cargo.tomls — both cheap, both propagatable downstream, and both would convert the over-claimed guarantees into real ones.

### spec-conformance

The machine conformance chain is in genuinely good shape: conformance-mapping.toml, the vendored snapshot, and conformance-report.json all agree at spec v0.8.0 with the exact upstream 40-requirement ID set; `just conform` passes 40/40 with 0 feedback signals and exit 0; the report was verified byte-stable across regenerations with no timestamp; and the gates the repo brags about are real — spec-version mismatch, unmapped requirement, unanchored met, and report drift all hard-fail (verified in code and unit tests). Spot-checks of 17 'met' claims across all six categories found the cited evidence overwhelmingly real: all 7 trybuild violation tests exist, the OUT-001/003/004 unit tests and cli.rs JSON/audit/version/catalog integration tests exist and assert what's claimed, the catalog is genuinely derived from the same clap tree the parser uses (meeting AGENT-006's required core), AGENTS.md is provider-neutral, the release workflow gates on conform, and the coverage gate carries its documented exclusion. The defects are concentrated in drift, not substance: CONFORMANCE.md was left at v0.7.0/39-requirements by the 0.8.0 bump commit despite its own 'corrected to match' promise (high); and an adversarial test proved the anchoring gate checks anchor *presence*, not *validity* — a dangling violation-test path passes green with only a buried signal, a weakness already manifest in the shipped mapping (a renamed audit test still cited by its old name) plus two more stale-free-text instances (ARCH-006's compile-time overclaim contradicting the prose, ENF-002's 3x-understated honor-system count). Net: conformance substance is solid; the trust artifacts' free-text layer rots faster than the generator can currently see, which is precisely the Principle 10 feedback the next iteration should mechanize.

### manifesto

Manifesto adherence is genuinely strong where it is mechanized, and weakest exactly where the repo itself predicted: prose. Strengths are real and verifiable: architecture rules sit at Principle 9's top rung (Cargo crate boundaries + 7 trybuild violation tests), `just check` is a true single gateway that CI invokes rather than duplicates, the AGENT-006 catalog is derived from the live clap tree (SSOT by construction, per crates/cli/src/catalog.rs), the conform generator mechanizes Truth-Seeking (ENF-008 anchoring gate, deterministic ENF-010 report sync-check, hermetic snapshot + spec-drift watcher), framework_purity tests protect the vendored .ckeletin/ from project-name leakage before it multiplies downstream, Principle 10 has concrete evidence (docs/spec-feedback-2026-05.md, six cross-implementation observations, the recorded hermetic-vs-live divergence), and Principle 4 is explicitly honored by deferring the plug-point patterns until a second consumer exists. The failures cluster in one pattern: human-authored truth lagging machine truth — CONFORMANCE.md is a full spec version stale despite promising 'prose can no longer silently diverge' (high), two enforcement-level claims are overstated in the published machine artifact (OUT-005's 'compile-time / cannot write to stdout' is factually false and the no-println rule sits at honor-system when a clippy lint rung is free; ARCH-006 says compile-time in the mapping but design in the prose), one evidence string describes superseded live-fetch behavior, and the toolchain pin lives in ~12 hand-synced copies. None of these break runtime behavior; all of them erode the trust artifact this scaffold exists to publish, and all are fixable with the repo's own preferred move — extend the conform gate to check them.

### tests

Test quality here is genuinely strong and the coverage claim is honest: I ran `just coverage` and it passes at 98.96% lines (TOTAL 957 lines, 10 missed) against the 85% CKSPEC-TEST-002 gate, with the one exclusion (.ckeletin/conform build tooling) documented in the justfile and conformance mapping; the trybuild violation tests carry exact .stderr snapshots pinning precisely the right error (unresolved import, i.e. dependency absence) under a pinned toolchain, so they cannot pass for the wrong failure; all three commands (ping, version, catalog) have both human-mode and JSON-mode integration tests plus real failure-path tests (bad config in both modes, stderr cleanliness in JSON mode, audit on/off/first-run-notice); DI-over-mocking is real (writer injection, a custom MakeWriter to capture shadow-log events, zero mock frameworks — enforced by a Cargo.lock grep check); and init_smoke runs the full test suite of a freshly initialized project, which is the right strongest signal for a scaffold. The defects cluster at composition seams the suite doesn't cross: the untested config-json=true path conceals a confirmed live precedence defect (explicit --output text cannot override config json=true, contradicting the documented layering — the one high finding); the framework update mechanism's apply/rollback path and its CKELETIN_UPDATE_RESULT machine contract — the very thing that propagates to downstream repos and drives the autonomous-maintenance vision — is tested only on its refusal guards; the vendored .ckeletin/tests/violations propagate downstream but are compiled by nothing (SSOT drift risk); and by the repo's own capture-before-declare discipline, build provenance could silently degrade to 'unknown' and the audit log could silently lose shadow events with every test green. None of these undermine the day-to-day green gate; they are the next ring of guarantees an n=1-by-design framework repo should pin before consumers depend on them unattended.

### errors

Error handling in this scaffold is well above average and most of it was empirically verified: config errors are exemplary (explicit --config missing → loud 'config file not found', default-location missing → silent defaults by documented design, malformed TOML → parse error with file/line/column, env-var type errors name the offending key and prefix — all exit 1); the three-stream contract holds under test (human errors → stderr only, JSON errors → envelope on stdout naming the failing subcommand via an exhaustive match with no silent fallback); init-time audit failures are loud; and the Justfile update recipes are a model of failure design (tier-1 rollback, tier-2 fix-forward with machine-readable CKELETIN_UPDATE_RESULT verdicts, trap-based restore in check-compatibility). The serious weakness is concentrated in the audit stream's own failure modes, which contradict the CKSPEC-OUT-004 MUST that the rest of the design takes pride in: error envelopes are never shadow-logged because the LogGuard is dropped before main()'s error renderer runs (proven: a failed run left an output.success record and no output.error in the audit file — the trail actively misreports a failure as success), and a valid or typo'd log_file_level silently empties the audit file with no warning. Secondary propagation risks: init.sh's seds are silent no-ops on pattern drift before the history-destroying step, and the `just test` recipe discards nextest's stderr and converts nextest failures into a full cargo-test rerun that can mask flaky tests. Fixing the guard lifetime, the shadow-log event level/filter, and those two vendored-framework recipes would close essentially every silent-failure path found.

### security

Security posture is genuinely strong for a template repo, with several practices well above baseline: workflows are free of script injection (untrusted values flow through env vars — release.yml uses ${GITHUB_REF_NAME}, spec-drift passes step outputs via env: mappings — never ${{ }} interpolated into run blocks); deny.toml locks sources to crates.io with unknown-registry/git denied and a tight permissive-only license allowlist; secret scanning is layered (pinned gitleaks 8.30.1 full-history CI gate + staged pre-commit scan, with the gitleaks-action EULA deliberately avoided); there is a weekly RUSTSEC heartbeat against the frozen lockfile, SBOM generation gated by grype at High+, Dependabot covering both cargo and github-actions, a cache-free --locked release build, and an unusually transparent audit-log UX (first-run notice, --no-audit, config off-switch). The real gaps cluster in supply-chain pinning consistency and propagated defaults: actions are tag-pinned, not SHA-pinned, while grype is installed via curl|sudo sh from an unpinned main branch — both contradicting the repo's own explicitly stated pinning philosophy; ci.yml lacks any permissions block (the other two workflows have one); and the vendored framework's spec-mandated shadow log lands every rendered byte in a world-readable 0644 file with no redaction or hardening hook (verified at runtime) — harmless for ping, but a default that every downstream consumer inherits for real data. Nothing found rises to critical or high: there is no exploitable injection, no over-privileged token in a code-execution path with write scope, and no broken guarantee a user will hit today — the findings are hardening drift that a template multiplies, which is exactly why the four mediums are worth fixing before more consumers fork.

### framework-dx

The .ckeletin/ developer-experience surface is genuinely strong in its newest layers: the catalog command (CKSPEC-AGENT-006) is verified to derive from the live clap tree through the OUT-002 envelope exactly as the spec demands (hidden help/version excluded, boolean-switch possible_values noise suppressed, schema shared as a framework type so consumers cannot emit a wrong shape); doctor/check-update/update all emit valid machine-readable verdicts I parsed successfully; the upstream/consumer self-guards (update, conform) encode real downstream incidents (triz, workhorse) with hermetic regression tests; init.sh plus the init_smoke CI job give an honest end-to-end guarantee that a fresh init yields a working, committed, v0.0.0-tagged project; and VERSION bumping has been disciplined on every framework PR since #16. The serious problems are concentrated in the update mechanism's git plumbing, where I empirically demonstrated that `git checkout <ref> -- .ckeletin/` neither deletes upstream-removed files (breaking the documented 'wholesale replacement' guarantee, latent until the first upstream file deletion) nor — used as rollback/restore — removes upstream-added files, which breaks check-compatibility's 'no changes kept' promise on the common file-adding release, blocks the subsequent update at its own pre-flight, and falsifies the `rolled_back:true` machine verdict; both are fixable with `git restore --source --staged --worktree` (fix verified in sandbox). Around that core sit medium-grade drift: tag-pinned updates fail on an invalid ref form the design doc promises, the documented migration mechanism is entirely unimplemented, ckeletin-health is a warning that the design doc miscredits as CI tamper enforcement, `just init` is the one destructive propagating recipe without the slug guard its siblings all have, and VERSION-bump discipline (with one proven miss) relies on convention rather than a CI gate.

### docs

Documentation accuracy is strong where it was engineered to be and rotting exactly where it is unguarded. Verified strengths: the README quickstart runs verbatim (both ping invocations executed successfully), every command in the AGENTS.md Commands table exists with correct package names (including the fragile `cargo test -p ckeletin --lib output::tests::envelope_success`, which passes), the Output::message() contract (`data: {\"message\": msg}` at .ckeletin/crate/src/output.rs:115) and the subcommand_name exhaustive-match convention (main.rs:72-78, no default arm) match AGENTS.md exactly, all 10+ test names cited across CHANGELOG/CONFORMANCE exist verbatim, and the framework changelog plus dated/status-marked plan, spec, and spec-feedback docs are exemplary. The failures cluster around one root cause: machine-validated artifacts (mapping, report, catalog) cannot drift, while prose has no gate — so CONFORMANCE.md fell behind the mapping (v0.7.0/39 vs v0.8.0/40) one commit after the anti-drift machinery landed, AGENTS.md still describes the pre-April crate layout and the twice-wrong '~20 lines' figure, and the root CHANGELOG missed the entire #16–#26 wave. The fix pattern is the one the repo already believes in (Principle 9 — Automated Enforcement): extend `just conform` to spot-check the prose headers the same way it sync-checks the JSON report.

---

## Confirmed findings

## Severity: high

### Reachable panic (exit 101) when the audit log file cannot be created in an existing directory

- **File:** `.ckeletin/crate/src/logging.rs:134`
- **Ref:** CKSPEC-OUT-004
- **Evidence:** prepare_file_appender guards only `std::fs::create_dir_all(log_dir)?` then calls `tracing_appender::rolling::daily(log_dir, log_name)`, which panics on file-creation failure. Verified: `chmod 555 $DIR; CKELETIN_LOG_FILE_PATH=$DIR/app.log ckeletin-rust ping` -> exit 101, stderr: `thread 'main' panicked at tracing-appender-0.2.5/src/rolling.rs:156:14: initializing rolling file appender failed: InitError { context: "failed to create log file", source: Os { code: 13, kind: PermissionDenied } }`. The existing test `prepare_file_appender_fails_on_invalid_path` only covers the create_dir_all failure (returns Err); the existing-but-unwritable case bypasses it. [dims: correctness]
- **Why it matters:** Audit logging is ON by default, so this panic fires before any command runs. Real-world trigger: run the CLI once under sudo on macOS (sudo preserves $HOME) -> root-owned `~/.config/<app>/logs/app.log.<date>` -> every subsequent user invocation panics with exit 101 and a raw backtrace instead of the documented error envelope / exit 1. This is framework code in .ckeletin/ that propagates to every downstream repo via `just ckeletin-update`.
- **Recommendation:** Use `RollingFileAppender::builder().rotation(Rotation::DAILY).filename_prefix(...).build(log_dir)` which returns `Result<_, InitError>`, and map the error into the existing io::Error return path (it already flows to a clean envelope + exit 1). Add a regression test with a 0o555 temp dir.
- **Verification:** confirmed, confirmed, confirmed

### Explicit `--output text` cannot override `json = true` from config or env — documented precedence broken (and the config.json branch is untested)

- **File:** `crates/cli/src/main.rs:86`
- **Ref:** CKSPEC-OUT-002; Principle 7 — Single Source of Truth
- **Evidence:** `json_mode = matches!(cli.output, root::OutputFormat::Json) || config.json` combined with root.rs:8 `#[arg(long, global = true, default_value = "text")]` makes an explicit `--output text` indistinguishable from the default. Verified: `CKELETIN_JSON=true ckeletin-rust --output text ping` and a cwd config.toml with `json = true` plus `--output text` both emit the JSON envelope. AGENTS.md line 13 promises 'defaults → TOML file → environment variables → CLI flags', and the code comment (main.rs:84-85) claims 'CLI flag overrides config'. crates/cli/tests/cli.rs contains zero tests that set `json = true` via config file or CKELETIN_JSON env — the `config.json` branch of this OR is never exercised at integration level, which is exactly why the defect survived. [dims: correctness,tests]
- **Why it matters:** The layered-precedence contract (CLI flags highest) is the scaffold's teaching pattern; downstream repos copy it. A user or agent stuck with CKELETIN_JSON=true in the environment has no flag-level escape back to human output, and the comment in the code asserts the opposite of actual behavior.
- **Recommendation:** Add integration tests: (1) config json=true → ping emits a JSON envelope; (2) config json=true + explicit `--output text` → human output. Test (2) fails today; fix by making the flag `output: Option<OutputFormat>` with no default, or by detecting explicit flag use via clap's ValueSource (matches.value_source("output") == Some(ValueSource::CommandLine)) so the CLI layer truly overrides config in both directions.
- **Verification:** confirmed, confirmed, confirmed

### Error envelopes are never shadow-logged: the audit-log worker is dead before Output::error runs

- **File:** `crates/cli/src/main.rs:133`
- **Ref:** CKSPEC-OUT-004
- **Evidence:** `let _guard = logging::init(&log_config)?;` is local to `run_inner` (main.rs:133). When a handler fails, `run_inner` returns Err, dropping `_guard` — which shuts down tracing-appender's non-blocking worker — BEFORE `run()` calls `output.error(...)` at main.rs:42. The `tracing::debug!(... "output.error")` event in .ckeletin/crate/src/output.rs:136 is then sent to a closed channel and silently discarded. Empirically verified: forced an EPIPE on a post-init write (`ping > fifo` with the reader closed) → exit=1, stderr showed `Error: Broken pipe (os error 32)`, but `grep output.error /tmp/ckaudit/audit.log*` → "NO output.error IN AUDIT LOG" while `output.success` events from the same process DID land. Output::error's doc comment (output.rs:128) promises "Both modes: shadow log to audit stream (CKSPEC-OUT-004)". [dims: errors]
- **Why it matters:** CKSPEC-OUT-004 is a MUST: every user-facing output operation must simultaneously log to the audit stream. Errors are the audit events that matter most when investigating failures, and the scaffold's wiring guarantees they are dropped — in every downstream repo, since real consumer commands fail post-init routinely. The unit tests in output.rs (capture_shadow_log) pass because they install their own subscriber, so the gap is invisible to `just check`. The user sees the error on stderr/envelope, but never learns the audit trail is missing it. (Pre-init errors — config load failures — also leave no audit record, but that is inherent; the post-init case is a fixable wiring defect.)
- **Recommendation:** Restructure main.rs so the LogGuard outlives error rendering: initialize logging (or at least hold the guard) in `run()`, e.g. have `run_inner` return the guard alongside the result, or split config/logging setup out of `run_inner` so `output.error` executes while the audit worker is alive. Add an integration test that forces a post-init command failure and asserts an `output.error` event exists in the audit file.
- **Verification:** confirmed, confirmed, confirmed

### ckeletin-update apply step is not 'wholesale replacement' — files deleted upstream persist forever in consumers

- **File:** `.ckeletin/Justfile:229`
- **Ref:** Principle 9 — Automated Enforcement; docs/specs/2026-04-14-framework-update-mechanism.md ('Replaced wholesale on update')
- **Evidence:** Recipe: `git checkout ckeletin-upstream/{{version}} -- .ckeletin/`. Sandbox proof (real git, consumer at old framework, upstream deleted .ckeletin/oldfile.txt): after `git checkout origin/main -- .ckeletin/` → "VERSION now: 0.2.0; oldfile (deleted upstream) still present? YES-STALE". The spec doc's ownership table promises `.ckeletin/` is "Replaced wholesale" on update. `git log --diff-filter=D -- .ckeletin/` is currently empty, so no consumer has been hit yet — the defect is latent until the first upstream file deletion/rename. [dims: framework-dx]
- **Why it matters:** `git checkout <ref> -- <path>` overwrites/adds files from the ref but never deletes files absent from it. The first time upstream removes or renames anything under .ckeletin/ (e.g. an obsolete test in .ckeletin/crate/tests/, which is a compiled test target), every consumer's update keeps the stale file, `git add .ckeletin/` commits it, and ckeletin-health can never flag it (it's committed, porcelain-clean). A stale test referencing a removed API would make the update falsely report compile_failed/check_failed; a stale config silently changes behavior. Defects here multiply across all downstream repos.
- **Recommendation:** Replace the apply step with `git restore --source=ckeletin-upstream/{{version}} --staged --worktree -- .ckeletin/` — empirically verified in the sandbox to delete upstream-removed files ("oldfile gone? YES"). Add a regression test in the update_guard family that updates a consumer copy across a commit that deletes a .ckeletin/ file.
- **Verification:** confirmed, confirmed, confirmed

### Rollback/restore via `git checkout HEAD -- .ckeletin/` leaves upstream-added files staged — breaks check-compatibility's 'no changes kept' promise and falsifies the rolled_back verdict

- **File:** `.ckeletin/Justfile:318`
- **Ref:** Principle 1 — Truth-Seeking (machine verdict must be true); spec doc 'rollback is automatic on failure' trust anchor
- **Evidence:** check-compatibility: `restore() { git checkout HEAD -- .ckeletin/ && cargo generate-lockfile; }; trap restore EXIT` (lines 318-319); update rollback uses the same `git checkout HEAD -- .ckeletin/` then prints `"rolled_back":true` (lines 236-238). Sandbox proof: after pulling an upstream state that adds .ckeletin/newfile.txt, `git checkout HEAD -- .ckeletin/` leaves `A  .ckeletin/newfile.txt` staged ("newfile still present after rollback? YES-LEFTOVER"). Recent framework releases DO add files (0.2.16 added .ckeletin/crate/src/catalog.rs and tests/conform_guard.rs), so the trigger is the common case. [dims: framework-dx]
- **Why it matters:** The documented flow is check-compatibility first, then update. On any release that adds files (nearly all of them), check-compatibility exits "Compatible — ... Run: just ckeletin-update" while silently leaving the added files staged; the subsequent `just ckeletin-update` then aborts at its own pre-flight ("Error: .ckeletin/ has uncommitted changes"), stranding the agent/user. On the update compile-failure path, `CKELETIN_UPDATE_RESULT={"status":"compile_failed",...,"rolled_back":true}` is false — exactly the machine-readable lie that breaks the autonomous-maintenance loop this surface was built for (0.2.14 changelog: 'speaks machine').
- **Recommendation:** Use `git restore --source=HEAD --staged --worktree -- .ckeletin/` in both restore() (line 318) and the tier-1 rollback (line 237) — verified in sandbox to remove the staged added file and leave porcelain clean. Add a consumer-simulation test asserting `git status --porcelain .ckeletin/` is empty after a check-compatibility run across a file-adding release.
- **Verification:** confirmed, confirmed, confirmed

### AGENTS.md architecture diagram shows config.rs/logging.rs/output.rs in crates/infrastructure/src/ — they moved to .ckeletin/crate/src/ in April; .ckeletin/ is never mentioned

- **File:** `AGENTS.md:25`
- **Ref:** CKSPEC-AGENT-004; Principle 7 — SSOT
- **Evidence:** AGENTS.md:25-30 diagram: 'infrastructure/ ... ├── config.rs # figment layered config ├── logging.rs ... └── output.rs # Envelope, human/JSON rendering, shadow log'. Actual: crates/infrastructure/src/ contains ONLY lib.rs, whose entire body is re-exports ('pub use ckeletin::config; pub use ckeletin::logging; pub use ckeletin::output; ...'); the real modules live in .ckeletin/crate/src/{config,logging,output,build_info,catalog,process}.rs since commit a7a6c08 'feat: restructure to .ckeletin/ framework model' (2026-04-15). grep '\.ckeletin|ckeletin-update' AGENTS.md CLAUDE.md README.md = zero hits. Also AGENTS.md:8 claims 'Workspace with 3 crates' while Cargo.toml members = ["crates/*", ".ckeletin/crate", ".ckeletin/conform"] (5 members). AGENTS.md was edited as recently as 2026-06-03 (843aed2) without fixing this. [dims: docs]
- **Why it matters:** AGENTS.md is the agent contract (CLAUDE.md: 'it contains all project knowledge'), and its directory-structure section describes a layout that has not existed for ~7 weeks. Worse, the vendored-framework model is completely undocumented here: an agent that greps its way to .ckeletin/crate/src/output.rs and edits it will have the change silently clobbered by the next `just ckeletin-update` — and this file propagates the same blind spot to every downstream consumer repo.
- **Recommendation:** Redraw the diagram to match reality: infrastructure/src/lib.rs (re-exports only) plus a .ckeletin/ entry labeled 'vendored framework — framework-owned, replaced wholesale by just ckeletin-update; do not edit'. Add cli/src/version.rs and catalog.rs to the cli subtree, and state the workspace has 5 members (3 project crates + 2 framework crates).
- **Verification:** confirmed, confirmed, confirmed

### LICENSE-APACHE is an abridged paraphrase, not the Apache License 2.0 — the patent-litigation termination clause is entirely missing

- **File:** `LICENSE-APACHE:63`
- **Ref:** Principle 1 — Truth-Seeking
- **Evidence:** Repo file is 6,573 bytes vs 9,723 for the canonical text (diffed against serde-1.0.228's LICENSE-APACHE). Repo §3 ends at '...with the Work to which such Contribution(s) was submitted.' — the entire defensive-termination sentence ('If You institute patent litigation ... then any patent licenses granted to You under this License for that Work shall terminate as of the date such litigation is filed.') is absent, even though the section still says 'irrevocable (except as stated in this section)', which now refers to nothing. Also missing/altered: §1 'Contribution' drops the whole '"submitted" means any form of electronic, verbal, or written communication...' sentence; §4 drops the 'You may add Your own copyright statement...' paragraph; §5 drops 'Notwithstanding the above, nothing herein shall supersede or modify the terms of any separate license agreement...'; §6 drops the 'except as required for reasonable and customary use...' carve-out; §7 (line 104) drops 'either express or implied, including, without limitation, any warranties or conditions of TITLE, NON-INFRINGEMENT, MERCHANTABILITY, or FITNESS FOR A PARTICULAR PURPOSE'; §8 drops the gross-negligence carve-out and 'even if such Contributor has been advised...'; §9 drops the indemnify/defend/hold-harmless condition; 'END OF TERMS AND CONDITIONS' is absent. Meanwhile Cargo.toml:11 declares `license = "MIT OR Apache-2.0"` (SPDX, inherited by crates/{cli,domain,infrastructure} via license.workspace), .ckeletin/crate/Cargo.toml:5 and .ckeletin/conform/Cargo.toml:5 declare the same, and README.md:39 says 'Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE)'.
- **Why it matters:** Apache-2.0 must be reproduced verbatim — a modified text is legally a different (unnamed) license, and the SPDX expression 'Apache-2.0' in five manifests plus the README points at a text that is not Apache-2.0. The dropped patent-retaliation clause and §9 indemnification condition materially change the granted terms. As a template, this file propagates verbatim into every derived project (init.sh never touches it), so the defect multiplies downstream exactly like a framework bug. Mitigation: the MIT half is verbatim and intact, so users electing MIT are unaffected — but anyone relying on the Apache patent grant has ambiguous terms.
- **Recommendation:** Replace LICENSE-APACHE with the verbatim canonical text from https://www.apache.org/licenses/LICENSE-2.0.txt (or copy serde's LICENSE-APACHE), keeping the appendix-style copyright/boilerplate block at the end. Optionally add a framework_purity-style test asserting the license file's hash/length matches canonical so the trust artifact can't silently drift again.
- **Verification:** confirmed, confirmed, confirmed

## Severity: medium

### Config/env-activated JSON mode is ignored on the error path — human error on stderr, empty stdout

- **File:** `crates/cli/src/main.rs:23`
- **Ref:** CKSPEC-OUT-002
- **Evidence:** run() computes `let json_mode = matches!(cli.output, root::OutputFormat::Json);` (flag only, line 23) while run_inner() computes `matches!(cli.output, Json) || config.json` (line 86). Verified: `CKELETIN_JSON=true CKELETIN_LOG_FILE_PATH=/dev/null/x/app.log ckeletin-rust ping` -> exit 1, stdout EMPTY, stderr `Error: Not a directory (os error 20)`. Same failure with `--output json` instead correctly emits {"status":"error","command":"ping","error":"Not a directory (os error 20)"} on stdout. Successes under CKELETIN_JSON=true DO emit JSON envelopes — so the binary is mixed-mode. [dims: correctness]
- **Why it matters:** CKSPEC-OUT-002 (MUST) says machine mode is 'activated via flag or configuration' and when active stdout MUST emit structured data. main.rs's own comment (line 34) promises 'errors in JSON mode MUST be JSON envelopes on stdout'. An agent that activates JSON via config/env (spec-blessed) parses stdout and gets nothing on any post-config-load failure (logging init, future command runtime errors in downstream repos). This is exactly the agent-drivability contract the project exists to provide.
- **Recommendation:** Resolve the output mode once. E.g., load config (or at least the json field) before the match in run(), or have run_inner return the resolved OutputMode alongside its Result so the error renderer in run() uses flag-OR-config; keep flag-only as the fallback solely for errors that occur before config load succeeds.
- **Verification:** confirmed, confirmed, downgraded

### Shadow logging silently disabled by log level config — valid `log_file_level = "info"` or any typo empties the audit stream with no warning

- **File:** `.ckeletin/crate/src/logging.rs:38`
- **Ref:** CKSPEC-OUT-004
- **Evidence:** Shadow events are emitted at DEBUG (output.rs:81/108/136) and the file filter is built with `EnvFilter::try_new(level).unwrap_or_else(|_| EnvFilter::new(fallback))` (logging.rs:38) — a silent fallback, and non-level strings parse as target directives so they never even hit the fallback. Empirically: `CKELETIN_LOG_FILE_LEVEL=info ckeletin-rust ping` → exit 0, audit file created but EMPTY (no output.success). Typo `CKELETIN_LOG_FILE_LEVEL=debgu` → same: exit 0, empty stderr, empty audit file. Typo'd console level `CKELETIN_LOG_LEVEL=inof` → no warning either. [dims: correctness,errors]
- **Why it matters:** A MUST-level guarantee (CKSPEC-OUT-004: 'The audit log MUST contain at least the data that was rendered to the user... always active regardless of output mode') hinges on a free-form config string with no validation and no feedback. 'info' is a level an operator plausibly sets to reduce noise — and it silently kills the entire shadow-log contract while the audit file still exists and looks healthy. The breakage is discovered exactly when the audit trail is needed (incident debugging).
- **Recommendation:** Emit shadow events at INFO (they are the primary purpose of the file stream), or pin the file layer's filter so `ckeletin::output` events always pass regardless of configured level (e.g. `EnvFilter::new(format!("{level},ckeletin::output=debug"))`). Separately, make `build_filter` warn to stderr when `EnvFilter::try_new` fails instead of silently substituting the fallback, and reject strings that aren't bare levels.
- **Verification:** confirmed

### OUT-005 enforcement claim is false: domain CAN write to stdout — 'compile-time' claim published in conformance report

- **File:** `conformance-mapping.toml:253`
- **Ref:** CKSPEC-OUT-005; CKSPEC-ENF-006; CKSPEC-ENF-002; Principle 9 — Automated Enforcement; Principle 1 — Truth-Seeking
- **Evidence:** Mapping (and the published conformance-report.json the spec repo aggregates) claims: enforcement_level = "compile-time", evidence = "Domain crate has no std::io path. Cannot write to stdout." This is untrue: println!/eprintln! are std-prelude macros needing no dependency — verified by compiling `pub fn execute() { println!(...); eprintln!(...); }` as a dependency-free lib with rustc (compiles clean). The cited violation test (domain_imports_infrastructure.rs) tests crate imports, not stream writes. No clippy::print_stdout/print_stderr lint is configured anywhere: grep for print_stdout/print_stderr across .ckeletin/Justfile and Justfile returns nothing, and no [lints] section exists in any Cargo.toml. AGENTS.md:121 ('Never `println!` or `eprintln!` in domain or infrastructure') and CLAUDE.md are therefore unenforced. The spec's own rationale (spec/04-output.yaml:108-112) says this is 'checkable by static analysis (detect direct print/stdout usage in business logic packages)'. [dims: architecture,manifesto]
- **Why it matters:** A consumer or auditor reading the published machine-readable report believes a domain `println!` cannot ship; in fact it compiles, passes clippy, passes all 7 violation tests, and passes `just check`. The enforcement guarantee is broken even though current domain code is clean (grep of crates/domain/src finds zero stream writes). ENF-006 requires proof that the claimed mechanism works — the cited proof proves a different mechanism. ENF-002's ladder has an unused feasible rung (lint).
- **Recommendation:** Add `[lints.clippy] print_stdout = "deny"` and `print_stderr = "deny"` to crates/domain/Cargo.toml (and infrastructure plus the framework crate), or add the -D flags scoped per-package in the ckeletin-clippy recipe. Correct the mapping entry to enforcement_level = "lint" with the real mechanism, regenerate conformance-report.json, and add a violation proof (a script test that runs clippy against a print-containing fixture, or violation_evidence naming the lint config).
- **Verification:** confirmed, downgraded, confirmed

### ARCH-006 enforcement level contradicts the audit table: mapping says 'compile-time', CONFORMANCE.md says 'design', and the violation_evidence is factually wrong

- **File:** `conformance-mapping.toml:61`
- **Ref:** CKSPEC-ARCH-006; CKSPEC-ENF-002; CKSPEC-ENF-004; CKSPEC-ENF-008; Principle 7 — SSOT; Principle 1 — Truth-Seeking
- **Evidence:** conformance-mapping.toml:61 has `enforcement_level = "compile-time"` for ARCH-006 and conformance-report.json publishes "compile-time"; but CONFORMANCE.md:39 (the ENF-004 audit table, which claims to be 'reconciled with conformance-mapping.toml') says `design`, as does the audit table row at :146. The mapping's violation_evidence — "Structural: only crates/cli/Cargo.toml has [[bin]] target" — is (a) false: .ckeletin/conform/Cargo.toml:8-10 declares `[[bin]] name = "conform"`, and (b) a non sequitur: having one bin target does nothing to keep main.rs minimal. `git log -L` shows the mapping block was originally "design" (07a75ac) and flipped to "compile-time" in 8892efe. The twin correction for ARCH-007 (compile-time → design) WAS applied to the mapping (line 70) but ARCH-006 was missed. Nothing at compile time prevents a 500-line business-logic main.rs from building — the [[bin]] structure enforces the entry point's location, not its minimality. [dims: architecture,spec-conformance,manifesto,docs]
- **Why it matters:** The mapping is the declared SSOT and feeds the published report the spec repo aggregates — so the wrong enforcement level (compile-time, when nothing at compile time prevents business logic in main.rs) is the one propagating cross-implementation, while the honest level lives only in the human document. ENF-008 exists precisely so 'met' claims carry real anchors; this anchor doesn't support the claim, and the two published trust artifacts now disagree on the precise thing enforcement_level exists to make visible.
- **Recommendation:** Change ARCH-006 in the mapping to enforcement_level = "design" (mirroring the ARCH-007 correction and matching CONFORMANCE.md), rewrite the violation_evidence to describe what actually constrains the entry point (review + the bootstrap-only convention), regenerate conformance-report.json via `just conform-report`, and note the correction in CONFORMANCE.md as was done for ARCH-007. Consider a conform lint: any entry whose violation_evidence starts with 'Structural' cannot claim compile-time.
- **Verification:** confirmed

### Entry layer imports clap directly, contradicting CKSPEC-ARCH-003's Entry-layer clause; Entry/Command split inside the cli crate is unenforced

- **File:** `crates/cli/src/main.rs:58`
- **Ref:** CKSPEC-ARCH-003; Principle 8 — Separation of Concerns
- **Evidence:** spec/01-architecture.yaml:62-63: "The Entry layer MAY import the Command layer but MUST NOT import the CLI framework directly." main.rs (documented as the Entry layer: AGENTS.md:33 'main.rs # Bootstrap only') contains `use clap::{CommandFactory, FromArgMatches};` at line 58 inside parse_args(). The mapping claims ARCH-003 met at compile-time, but its trybuild tests only cover the domain/infrastructure boundary; the Entry-vs-Command split lives inside one crate where no compile-time or lint mechanism applies — the clause is honor-system and currently violated in letter. [dims: architecture]
- **Why it matters:** The four-layer model collapses Entry and Command into the cli crate — a reasonable Rust realization — but the spec clause about Entry not touching the framework is then neither met nor documented as a divergence. Downstream scaffold consumers copy this pattern verbatim.
- **Recommendation:** Move parse_args() (and the --version injection) into root.rs so root owns all clap surface and main.rs only calls root::parse(), restoring the spec letter cheaply; or, if the collapse is deliberate, say so in the ARCH-003 mapping evidence and note the intra-crate clause as honor-system per ENF-003.
- **Verification:** confirmed

### ARCH-004's 'business logic packages MUST NOT import each other' has no enforcement: all features are modules in one domain crate

- **File:** `crates/domain/src/lib.rs:1`
- **Ref:** CKSPEC-ARCH-004; Principle 9 — Automated Enforcement
- **Evidence:** domain is a single crate (`pub mod ping;`), and AGENTS.md:64 instructs new features to be added as sibling modules ('Domain logic (crates/domain/src/mycommand.rs)'). Rust modules within one crate can freely `use crate::ping` — nothing at compile time, lint, or test level prevents cross-feature imports. Yet conformance-mapping.toml:39-48 claims ARCH-004 met at enforcement_level = "compile-time" with evidence 'No cross-domain ... imports'. The claim is vacuously true today (one module) but the mechanism does not exist for the clause; the trybuild tests cited cover only external-dep isolation (figment/tracing). [dims: architecture]
- **Why it matters:** The moment a second domain module lands (the documented growth path), feature-to-feature coupling becomes possible with zero alarms, while the published report says the rule is compile-time enforced. ckeletin-go gets this per-package; the Rust realization silently downgrades it.
- **Recommendation:** Either adopt crate-per-feature under crates/domain/ (true compile-time isolation), or add an invariant test in the framework_purity.rs style that scans crates/domain/src/*.rs for `use crate::`/`use super::` referencing sibling feature modules — and split the ARCH-004 mapping claim so the cross-feature clause carries its real (script/test) level.
- **Verification:** confirmed

### Boundary enforcement is an enumerated denylist: a new forbidden dep in domain, or clap added to the vendored framework crate, is caught by nothing

- **File:** `.ckeletin/crate/Cargo.toml:8`
- **Ref:** CKSPEC-ARCH-003; CKSPEC-ARCH-004; Principle 9 — Automated Enforcement
- **Evidence:** The trybuild net enumerates exactly 5 crates for domain (clap, tracing, figment, infrastructure, ckeletin) and 2 for infrastructure (domain, clap). Adding e.g. `tokio` or `reqwest` to crates/domain/Cargo.toml [dependencies] passes cargo check, clippy, all violation tests, and conform. Worse for the propagated framework: .ckeletin/crate/Cargo.toml (deps: serde, serde_json, figment, tracing*, thiserror) has no guard at all — the trybuild manifest (target/tests/trybuild/infrastructure/Cargo.toml) shows transitive deps don't enter the test crate's extern prelude, so `infra_imports_clap` would STILL pass if clap were added to the ckeletin crate, and framework_purity.rs only greps source strings ('ckeletin-rust', env prefixes), never the dependency list. The infrastructure layer adopting the CLI framework would violate ARCH-003 in every downstream consumer with green checks everywhere. [dims: architecture]
- **Why it matters:** .ckeletin/ is the upstream source of truth that propagates to consumer repos via `just ckeletin-update` — a layering defect there multiplies across every downstream repo. The repo's own documented discipline (AGENTS.md capture-before-declare / registry-iterating invariant tests) argues for allowlist invariants, not enumerated denylists.
- **Recommendation:** Add an allowlist invariant test (natural home: .ckeletin/crate/tests/framework_purity.rs, which already does file-reading invariants): parse crates/domain/Cargo.toml and assert [dependencies] == {serde}; parse crates/infrastructure/Cargo.toml and assert == {ckeletin}; parse .ckeletin/crate/Cargo.toml and assert clap (and any CLI framework) is absent. This propagates downstream automatically with the framework.
- **Verification:** confirmed

### Vendored .ckeletin/tests/violations/ is compiled and run by nothing; the active violation tests are unpropagated project-owned copies with no drift guard

- **File:** `.ckeletin/tests/violations/domain_imports_clap.rs`
- **Ref:** CKSPEC-ENF-006; Principle 7 — SSOT; Principle 9 — Automated Enforcement
- **Evidence:** grep for 'ckeletin/tests' across all .rs/.toml/.sh/.yml/Justfile files finds references only in .ckeletin/CHANGELOG.md and docs/plans — no Cargo target, no just recipe, no CI job, and init.sh never copies them. conformance-mapping.toml cites the violation tests 8 times, all as crates/domain|infrastructure/tests/violations/* paths, never .ckeletin/tests. The 7 vendored .rs files are currently byte-identical to the crate copies (verified by diff), and the crate copies additionally carry the .stderr snapshots the vendored set lacks. The update recipe (`git checkout ckeletin-upstream/{{version}} -- .ckeletin/`, .ckeletin/Justfile:229) propagates only .ckeletin/, so an upstream tightening of a violation template (or a new violation file) reaches consumers' .ckeletin/ yet never their executing test suites. [dims: architecture,tests]
- **Why it matters:** Two copies of the enforcement tests exist with no mechanism keeping them aligned (Principle 7 — SSOT). Worse, the copy that propagates downstream via `ckeletin-update` is the dead one: if upstream strengthens a violation test in .ckeletin/tests/, consumers receive inert files while their live crates/*/tests/violations copies stay stale forever. The enforcement net downstream can silently fall behind the framework's own definition of the boundaries — precisely the multiplied-defect channel the vendored-framework design needs to guard.
- **Recommendation:** Either delete .ckeletin/tests/violations/ (and document that violation tests are project-owned, seeded once by the scaffold), or make it the SSOT: have the crate test drivers reference the vendored files (trybuild accepts paths), or add a framework_purity-style invariant test asserting crates/*/tests/violations/*.rs match .ckeletin/tests/violations/*.rs byte-for-byte so drift fails `just check` after an update until the consumer reconciles.
- **Verification:** confirmed

### CONFORMANCE.md left stale at spec v0.7.0 / 39 requirements after the 0.8.0 bump — breaking its own reconciliation promise

- **File:** `CONFORMANCE.md:1`
- **Ref:** CKSPEC-ENF-004; Principle 7 — SSOT; Principle 9 — Automated Enforcement
- **Evidence:** CONFORMANCE.md:1 '# Ckeletin Spec v0.7.0 — Rust Conformance Report', :4 'Spec version: 0.7.0', :6 'Total: 39 requirements — 39 met', :107 '## Agent Readiness (5/5 met)' with no CKSPEC-AGENT-006 row anywhere (grep AGENT-006 CONFORMANCE.md = no hits), and the ENF-004 audit table has no row for the catalog enforcement. Versus conformance-mapping.toml:5 'spec_version = "0.8.0"', conformance-report.json summary '{met: 40, total: 40, passed: true}', and `just conform` output 'PASSED — 40/40 requirements met'. Commit 91480da (2026-06-05, 'spec 0.7.0 -> 0.8.0') updated the mapping/report/snapshot but not CONFORMANCE.md (its last edit is 2596148, 2026-06-04). The file's own rule at lines 8-11 — 'When prose and mapping disagree, the mapping wins and this file is corrected to match' — was not followed, and README.md:33 sends readers here for 'the exact spec version, requirement count'. Nothing in `just conform` checks this file (grep -c 'CONFORMANCE.md' .ckeletin/conform/src/main.rs = 0). [dims: spec-conformance,manifesto,docs]
- **Why it matters:** CONFORMANCE.md is the published prose trust artifact, and the mapping's evidence for CKSPEC-ENF-002/003/004 explicitly points at its 'Enforcement Audit Table' as the artifact satisfying those MUST requirements. ENF-004 requires a *living* audit table; this one now states the wrong spec version, the wrong requirement count, and omits a requirement the repo claims to meet. The document also breaks its own stated reconciliation promise — the second occurrence of the exact 'conformance reporting rots faster than code' failure mode the report itself warns about — and stays green while wrong because nothing in `just conform` checks it.
- **Recommendation:** Update CONFORMANCE.md to v0.8.0: header, spec version, 40/40 total, an Agent Readiness 6/6 section with the AGENT-006 row, and a catalog row in the ENF-004 audit table; refresh the report date. Then close the gap structurally: add a cheap check to the conform generator (or the mapping's ENF-004 checks list) that greps CONFORMANCE.md for the mapping's spec_version and requirement total, so prose drift fails `just conform` instead of relying on the honor system.
- **Verification:** confirmed, confirmed, downgraded

### Conform gate validates anchor presence, not validity — a dangling violation-test path passes green

- **File:** `.ckeletin/conform/src/main.rs:456`
- **Ref:** CKSPEC-ENF-006
- **Evidence:** Empirical: in a scratch worktree of HEAD I pointed CKSPEC-ARCH-002's and CKSPEC-OUT-005's violation_tests at 'crates/domain/tests/violations/DOES_NOT_EXIST.rs', regenerated the report, and ran conform. Output: 'PASSED — 40/40 requirements met, 0 deferred. 2 feedback signal(s) for spec review.' with EXIT=0; the missing files only produced ENF-007 signals ('CKSPEC-ARCH-002: violation test not found: ...'). Code confirms: main.rs:456-465 pushes feedback for a missing file but only `failed_checks > 0` triggers exit(1) (lines 541-546, 561-563). Similarly, `lacks_anchor` (line 196) and `lacks_enforcement_proof` (line 182) accept any non-empty violation_tests list or any non-blank free-text violation_evidence — neither path existence nor cited-test reality is required to pass. [dims: spec-conformance]
- **Why it matters:** The repo's headline claim (CONFORMANCE.md line 69: 'a green report means every claim is anchored, not asserted') holds only for anchors that are *absent*, not anchors that are *stale or false*. If a violation test is renamed or deleted, CI conform stays green and the published report keeps citing it — the signal is buried in the logs of a passing job nobody reads. ENF-006 is a MUST ('claims above honor-system MUST be accompanied by proof that the enforcement mechanism works'); a dangling path is not proof. This is vendored framework code (.ckeletin/conform), so the weakness propagates to every downstream repo via ckeletin-update. Mitigating: the spec's own ENF-007 lists 'a violation test is missing' as a feedback-signal category, so signal-not-fail is a defensible reading — hence medium, not high.
- **Recommendation:** Make a declared violation_tests path that does not exist a hard failure (exit non-zero), like the unanchored-met gate — or at minimum make `feedback_signals > 0` fail the CI conform job while staying advisory locally. Consider also verifying that test names quoted in violation_evidence (a known free-text drift vector, see the OUT-004 finding) resolve via a `grep -rq` check.
- **Verification:** confirmed

### Stale evidence anchor: OUT-004 cites test `audit_log_written_by_default`, which no longer exists

- **File:** `conformance-mapping.toml:248`
- **Ref:** CKSPEC-ENF-008
- **Evidence:** conformance-mapping.toml:248 (CKSPEC-OUT-004 violation_evidence): 'default-on/opt-out integration tests audit_log_written_by_default + no_audit_flag_disables_the_log_file in crates/cli/tests/cli.rs'. Repo-wide grep for `audit_log_written_by_default` finds nothing; the actual test is `audit_log_written_under_config_home_by_default` at crates/cli/tests/cli.rs:216. `git log -S` shows the rename happened in commit cd105cf ('feat(output): default the audit log to ~/.config/<app> instead of cwd') without updating the mapping. The stale name is also propagated into the published conformance-report.json. [dims: spec-conformance]
- **Why it matters:** This is a live instance of the exact failure mode CKSPEC-ENF-008 exists to prevent: a verifier following the published anchor finds no such test. The requirement is genuinely met (the renamed test exists and asserts default-on audit logging, and the output.rs shadow-log tests at .ckeletin/crate/src/output.rs:406/421 are real), so conformance is intact — but the anchor is unverifiable as written, and the generator structurally cannot catch free-text drift.
- **Recommendation:** Correct the violation_evidence to `audit_log_written_under_config_home_by_default` and regenerate conformance-report.json via `just conform-report`. Longer term, prefer machine-checkable anchors (e.g. add `cargo test -p cli --test cli audit` to OUT-004's checks) over test names embedded in prose.
- **Verification:** confirmed

### CKSPEC-ENF-005 evidence string describes superseded behavior ('live, with vendored fallback') — the code is hermetic-by-default

- **File:** `conformance-mapping.toml:120`
- **Ref:** CKSPEC-ENF-005; Principle 1 — Truth-Seeking; Principle 7 — SSOT
- **Evidence:** conformance-mapping.toml:120: evidence = '...loads the spec requirement IDs (live, with vendored conformance/requirements.json fallback)...'. But .ckeletin/conform/src/main.rs:93 documents the opposite: 'Default (CI / gating): read ONLY the committed vendored requirements.json' (load_spec_requirements only fetches when --refresh is passed, lines 102-148), and .ckeletin/Justfile's conform recipe comment says 'hermetic: reads the committed vendored spec, no network'. CONFORMANCE.md:65 also says hermetic. The stale string is published verbatim in conformance-report.json. [dims: manifesto]
- **Why it matters:** The hermetic-vs-live design is the repo's headline documented divergence from ckeletin-go (CONFORMANCE.md observation #5); the SSOT mapping describing it backwards means anyone (or any agent) trusting the mapping over the prose gets the architecture wrong — and ENF-008's anchoring gate checks evidence presence, not evidence truth, so this can't self-heal.
- **Recommendation:** Rewrite the ENF-005 evidence string to match reality ('hermetic: reads the committed vendored snapshot; --refresh re-fetches') and regenerate the report.
- **Verification:** confirmed

### Rust toolchain version duplicated in ~12 locations with only a by-hand sync procedure (which itself omits release.yml)

- **File:** `rust-toolchain.toml:13`
- **Ref:** Principle 7 — SSOT; Principle 9 — Automated Enforcement
- **Evidence:** channel = "1.96.0" in rust-toolchain.toml; `dtolnay/rust-toolchain@1.96.0` appears 8x in .github/workflows/ci.yml and 2x in release.yml (grep -c: ci.yml:8, release.yml:2); Cargo.toml:10 rust-version = "1.96" ('kept in lockstep with rust-toolchain.toml' — comment only). rust-toolchain.toml's bump procedure says 'update the pinned version in .github/workflows/ci.yml to match' — omitting release.yml — while .github/dependabot.yml says 'rust-toolchain.toml → refresh the .stderr snapshots → match the ci.yml/release.yml pins'. No script or test checks the pins agree. [dims: manifesto]
- **Why it matters:** Twelve copies of one truth, synced by documented human discipline — exactly what Principle 9 says fails under time pressure. A missed pin yields a CI toolchain/component mismatch with rust-toolchain.toml (confusing trybuild/component failures), and the two written procedures already disagree about which files to update.
- **Recommendation:** Add a cheap automated sync check (a conform check or invariant test asserting every `dtolnay/rust-toolchain@X` in .github/workflows/*.yml matches the rust-toolchain.toml channel and that Cargo.toml rust-version is its major.minor), and fix the rust-toolchain.toml comment to include release.yml.
- **Verification:** confirmed

### main.rs line-count stated in two docs, three different numbers, both stale (AGENTS.md '~20', CONFORMANCE.md '102', actual 149)

- **File:** `AGENTS.md:33`
- **Ref:** Principle 7 — SSOT; Principle 1 — Truth-Seeking; CKSPEC-ARCH-006
- **Evidence:** AGENTS.md:33: 'main.rs     # Bootstrap only (~20 lines)'. CONFORMANCE.md:44: 'main.rs is the bootstrap entry... **102 lines**, 100% line-covered. (The earlier report's "~20 lines" figure was wrong.)'. `wc -l crates/cli/src/main.rs` → 149 (the version and catalog wiring landed after the note was written). The figure CONFORMANCE.md flagged as wrong still lives in AGENTS.md, and the 'corrected' 102 is itself now stale. [dims: manifesto,docs]
- **Why it matters:** A copied volatile fact (line count) drifted twice — the textbook Principle 7 case of copying instead of referencing, and the exact drift pattern the project already caught once and documented as an error, with the correction applied to CONFORMANCE.md only. Internal contradiction between the two primary docs erodes trust in both; agents reading AGENTS.md get a 7x-wrong size claim about the file the doc tells them to study, and the substantive ARCH-006 claim (bootstrap only) is demonstrably unmaintainable in hard-count form.
- **Recommendation:** Drop the numeric counts from both docs ('bootstrap only — parse, config, logging init, dispatch; no business logic' carries the meaning); if a number must exist, have conform compute it.
- **Verification:** confirmed

### Root CHANGELOG [Unreleased] is ~10 PRs behind — catalog command, spec 0.8.0, and the whole 2026-06-04/05 security/tooling wave are unrecorded

- **File:** `CHANGELOG.md:8`
- **Ref:** CKSPEC-CL-005; CKSPEC-CL-006; Principle 9 — Automated Enforcement
- **Evidence:** git log shows CHANGELOG.md last touched at 95b9d36; subsequent commits 93f7b42..91480da (PRs #16–#26) added: update compatibility check, doctor/version recipes, gitleaks secret scanning, SBOM + vuln scanning, cargo-geiger + security clippy lints, bolero fuzzing, the agent-drivable update surface, the `catalog` subcommand (crates/cli/src/catalog.rs + root.rs + main.rs — a user-visible new command), and the spec 0.7.0→0.8.0 conformance record. grep -i 'catalog|doctor|sbom|gitleaks|geiger|bolero|fuzz|0.8.0|AGENT-006' CHANGELOG.md = zero hits, and CHANGELOG.md:11 still describes conformance as 'spec v0.7.0 (39 requirements, all met)'. git show 4b5dbe6 touched .ckeletin/CHANGELOG.md but not CHANGELOG.md; 91480da touched neither. [dims: manifesto,docs]
- **Why it matters:** The repo claims CKSPEC-CL-005 (Unreleased section) and CL-006 (human-curated) as met, and its own established practice records exactly this class of change in [Unreleased]. The framework side IS meticulously recorded in .ckeletin/CHANGELOG.md, but a reader of the root changelog gets a repo state that ended on 2026-06-04 and never learns the binary grew a third subcommand — demonstrating Principle 9's prediction that unenforced rules erode; the next release's notes will silently omit the feature.
- **Recommendation:** Add [Unreleased] entries for the #16–#26 wave: at minimum the catalog command (CKSPEC-AGENT-006, framework 0.2.16), the security tooling additions (gitleaks/SBOM/geiger/fuzzing), the update-guard + agent-drivable diagnostics, and the spec 0.8.0 / 40-requirement conformance record. Consider a lefthook/CI nudge (warn when a feat:/fix: commit touches crates/ without touching CHANGELOG.md).
- **Verification:** confirmed

### The framework update apply path — two-tier rollback and the CKELETIN_UPDATE_RESULT machine contract — has no test; only the refusal guards are tested

- **File:** `.ckeletin/Justfile:233`
- **Ref:** Principle 2 — Automated Enforcement
- **Evidence:** ckeletin-update implements tier-1 rollback (`if ! cargo check --workspace; then ... git checkout HEAD -- .ckeletin/ ... printf 'CKELETIN_UPDATE_RESULT={"status":"compile_failed",...}'`) and tier-2 leave-in-tree (`if ! just check; then ... "status":"check_failed" ... exit 1`), plus the success path that commits and prints `"status":"updated"`. .ckeletin/crate/tests/update_guard.rs tests ONLY that the three recipes refuse to run on the upstream repo (the guard short-circuit); no test ever applies an update to a simulated consumer, exercises either rollback tier, or parses the CKELETIN_UPDATE_RESULT JSON. [dims: tests]
- **Why it matters:** This recipe IS the propagation mechanism — defects here multiply across every downstream repo. The CKELETIN_UPDATE_RESULT line is explicitly the contract for an autonomous driver (workhorse); a printf typo or shape change breaks the autonomous-maintenance loop with zero test signal. The rollback logic is the highest-risk shell code in the framework and is currently verified only by manual use.
- **Recommendation:** Add a hermetic test mirroring conform_guard's approach: rsync the scaffold to a temp dir, strip the upstream slug (consumer simulation), `git init` it, pre-add a `ckeletin-upstream` remote pointing at a local bare clone (the recipe only adds the remote if missing, so no network), then assert (a) a clean update commits and prints valid `{"status":"updated",...}` JSON (parse it with jq/serde_json), and (b) an upstream ref with a non-compiling .ckeletin/ triggers rollback and `compile_failed`. Even testing only the JSON contract of the no-op/refusal paths would be progress.
- **Verification:** confirmed

### Build identity can silently degrade to permanent 'unknown' with every test green — no pinned-capture regression test against real git, contrary to the repo's own capture-before-declare discipline

- **File:** `crates/cli/tests/cli.rs:42`
- **Ref:** CKSPEC-OUT-006
- **Evidence:** version_command_json_has_fields asserts only field names: `.stdout(predicate::str::contains("\"commit\":"))` etc.; version_flag_surfaces_build_identity asserts only the literal word "commit". Both pass identically whether build.rs bakes a real SHA or degrades to "unknown" (the documented honest-degradation path). The live binary currently bakes `commit 91480da, built 2026-06-05` — but no test would fail if a refactor of build.rs's `git describe --always --abbrev=7 --dirty --match=__ckeletin_no_such_tag__` invocation broke and 'unknown' shipped forever. unit tests split_dirty_sha pin hand-written literals ("6e75184-dirty"), not captures of real git output. [dims: tests]
- **Why it matters:** AGENTS.md's capture-before-declare discipline (earned through 'three separate incidents of constants picked from intuition drifting silently against the real system, each with green tests') applies squarely here: the '-dirty' suffix and SHA shape are external-system (git) constants, and the test suite cannot detect the exact failure mode the discipline warns about — silent degradation with green tests. CKSPEC-OUT-006 makes provenance a MUST.
- **Recommendation:** Add an integration test gated on the test environment being a git repo (it is, locally and in CI): build/run the binary and assert the JSON `commit` matches ^[0-9a-f]{7}(-dirty)?$ and `date` matches ^\d{4}-\d{2}-\d{2}$ — i.e., NOT 'unknown' when git is available. Optionally a build-script-level test running `git describe --dirty` in a temp repo with a staged change, pinning that real git emits the `-dirty` suffix.
- **Verification:** confirmed

### Audit log content is never asserted end-to-end — integration tests check only that the log directory exists

- **File:** `crates/cli/tests/cli.rs:216`
- **Ref:** CKSPEC-OUT-004
- **Evidence:** audit_log_written_under_config_home_by_default asserts `tmp.path().join("ckeletin-rust/logs").is_dir()` — directory existence only. No integration test reads the log file and asserts the rendered output landed there. The chain works today (live run: app.log contains {"level":"DEBUG","fields":{"message":"output.success","command":"ping","data":"Pong! ckeletin-rust is alive"}}), but the guarantee depends on main.rs:131 wiring `file_level: config.log_file_level` (default "debug") to Output's tracing::debug! events. A one-token typo in run_inner — passing config.log_level ("info") instead of config.log_file_level — would silently drop every shadow event from the audit file while all existing tests stay green (the unit shadow-log tests use their own subscriber; the config test pins the default value; nothing tests the composed pipeline). [dims: tests]
- **Why it matters:** CKSPEC-OUT-004 is a MUST: 'The audit log MUST contain at least the data that was rendered to the user.' The pieces are each tested, but the seam where they compose — exactly where silent breaks live — is not. The conformance report claims OUT-004 met partly on the strength of these tests.
- **Recommendation:** Extend one audit test: after `audit_cmd(tmp.path()).arg("ping")` succeeds, read the file(s) under <xdg>/ckeletin-rust/logs/ and assert the content contains "Pong! ckeletin-rust is alive" (drop/flush handled by process exit). One assertion closes the seam.
- **Verification:** confirmed

### ckeletin-health can never fail — 'Workspace: BROKEN' exits 0 inside the `just check` gate, yet the design doc claims it catches framework tampering

- **File:** `.ckeletin/Justfile:15`
- **Ref:** Principle 9 — Automated Enforcement; CKSPEC-ENF-001; docs/specs/2026-04-14-framework-update-mechanism.md §Testing Strategy
- **Evidence:** Recipe lines 17-23: dirty .ckeletin/ takes the else-branch that only echoes "WARNING: ..." (exit 0), and `@cargo check --workspace -q && echo "Workspace: compiles" || echo "Workspace: BROKEN"` always exits 0 — shell semantics verified: `sh -c 'false && echo ok || echo BROKEN'` prints BROKEN and exits 0; `check: ckeletin-check test ckeletin-health` (Justfile:9) then prints "All checks passed." The design doc claims: "`just check` includes `ckeletin-health` so CI pipelines catch local modifications" and "CI pipelines that run `just check` automatically catch framework tampering." In CI the checkout is the committed state, so `git status --porcelain .ckeletin/` is empty by construction — committed tampering is invisible to this check even if it did fail. Unlike `ckeletin-doctor`, whose header documents 'always exits 0 — informational only', ckeletin-health has no such contract and sits inside the gating recipe. [dims: errors,framework-dx]
- **Why it matters:** The enforcement claim is doubly hollow: locally it's a warning that scrolls past in `just check` output, and in CI it structurally cannot fire. A consumer that patches .ckeletin/ and commits gets green CI forever — until the next update silently overwrites the patch, which is the exact failure mode the boundary protection was designed to surface. An agent scripting `just ckeletin-health` gets exit 0 for a broken workspace; inside `check` it can print BROKEN followed by 'All checks passed.'
- **Recommendation:** Make the dirty-.ckeletin/ branch exit non-zero (or add a `ckeletin-health --strict` used by `just check`), let the compile probe's failure propagate (drop the `|| echo` arm or add `exit 1`), and for committed-drift detection ship a checksum manifest (or compare .ckeletin/ against `ckeletin-upstream` at the recorded VERSION) in a CI job. At minimum, correct the design doc's claim and document ckeletin-health as informational.
- **Verification:** confirmed

### GitHub Actions pinned to mutable tags, not commit SHAs

- **File:** `.github/workflows/ci.yml:33`
- **Ref:** Principle 9 — Automated Enforcement
- **Evidence:** All three workflows use tag refs: `uses: actions/checkout@v6`, `uses: actions/cache@v5`, `uses: dtolnay/rust-toolchain@1.96.0`, `uses: actions/upload-artifact@v4` (ci.yml:33,35,79,248; release.yml:30,33,46,49; spec-drift.yml:31). Zero SHA pins. Meanwhile ci.yml:18-21 states the repo's own philosophy: "Tool versions are PINNED, not floating" — but tags are exactly such floating refs: a tag can be force-moved by a compromised maintainer account (the tj-actions/changed-files incident, March 2025, exfiltrated secrets this way). [dims: security]
- **Why it matters:** A moved tag executes attacker code with the workflow's token in every repo using it. This is a template repo: the pattern propagates verbatim into every downstream clone, multiplying exposure. The repo already pins cargo tools and gitleaks by exact version for precisely this reason, so the actions refs are the one unpinned supply-chain edge left.
- **Recommendation:** Pin each action to a full commit SHA with a version comment, e.g. `uses: actions/checkout@08c6903cd8c0fde910a37f88322edcfb5dd907a8 # v6.0.0`. Dependabot's github-actions ecosystem (already configured) keeps SHA pins updated automatically, so maintenance cost is zero.
- **Verification:** confirmed

### grype installed via curl | sudo sh from an unpinned `main` branch script, no version, no checksum

- **File:** `.github/workflows/ci.yml:241`
- **Ref:** Principle 7 — Single Source of Truth
- **Evidence:** ci.yml:241: `run: curl -sSfL https://raw.githubusercontent.com/anchore/grype/main/install.sh | sudo sh -s -- -b /usr/local/bin` — the install script floats on anchore/grype@main AND the installed grype version floats to latest, executed as root, with no checksum verification. This directly contradicts ci.yml:18-21 ("Tool versions are PINNED, not floating") and the secret-scan job's own comment at ci.yml:195 ("Pinned version, like the other CI tools"). The gitleaks install (ci.yml:204-207) pins VERSION=8.30.1 but also verifies no checksum on the tarball before `sudo tar -xz -C /usr/local/bin`. [dims: security]
- **Why it matters:** A compromised anchore repo or a malicious change to install.sh on main executes arbitrary root code in the SBOM job on every push/PR — the job that produces the supply-chain artifact consumers are meant to trust. It is also a reproducibility hole: the grype version (and thus the vuln-DB gate behavior) silently changes under the workflow. Template propagation multiplies it.
- **Recommendation:** Pin the script to a release ref and pass an explicit version: `curl -sSfL https://raw.githubusercontent.com/anchore/grype/v0.x.y/install.sh | sh -s -- -b <dir> v0.x.y`, or better, download the versioned release tarball and verify its published sha256 (same fix for the gitleaks tarball: grab checksums.txt and `sha256sum -c`). Drop `sudo` by installing to a user-writable dir on PATH.
- **Verification:** confirmed

### Audit log is world-readable (0644/0755) and shadow-logs all rendered output with no redaction or hardening hook

- **File:** `.ckeletin/crate/src/logging.rs:132`
- **Ref:** CKSPEC-OUT-004
- **Evidence:** Verified at runtime: `XDG_CONFIG_HOME=$TMP ./target/debug/ckeletin-rust ping` creates `<config>/ckeletin-rust/logs/app.log.2026-06-09` as `-rw-r--r--` inside `drwxr-xr-x` dirs (umask 022). logging.rs:132 uses plain `std::fs::create_dir_all(log_dir)?` and tracing_appender's rolling writer — no permission narrowing anywhere. output.rs:81 `tracing::debug!(command = command, data = %data, "output.success")` shadow-logs the full rendered payload of every command, file_level defaults to "debug" (config.rs:53), enabled by default (config.rs:46-48), with no redaction mechanism and no retention/rotation cleanup (daily files accumulate forever). [dims: security]
- **Why it matters:** CKSPEC-OUT-004 mandates shadow logging, so by design every byte a downstream command ever displays — which in real consumer CLIs will include credentials shown by a `config show`, API responses, PII — lands in a world-readable file under ~/.config that persists indefinitely. The scaffold's ping output is harmless, but this is exactly the vendored pattern (.ckeletin/crate) that propagates to every consumer repo. Tools that write security-relevant per-user files (ssh, gh) use 0700/0600 for this reason. The first-run notice and --no-audit flag are good transparency, but don't fix who else on the machine can read the file.
- **Recommendation:** On Unix, create the log directory with mode 0700 (std::os::unix::fs::DirBuilderExt) and pre-create/chmod the log file to 0600 before handing it to tracing_appender; document a redaction convention (e.g. mark sensitive fields so Output skips or masks them in the shadow log) and a retention note in AGENTS.md so downstream authors inherit safe defaults, not just the MUST-log requirement.
- **Verification:** confirmed

### Tag-pinned updates are broken: `ckeletin-update version=v0.2.0` resolves `ckeletin-upstream/v0.2.0`, an invalid ref

- **File:** `.ckeletin/Justfile:229`
- **Ref:** docs/specs/2026-04-14-framework-update-mechanism.md §Update Flow step 5 ('a branch name or tag like v0.2.0')
- **Evidence:** `ckeletin-update` and `ckeletin-update-dry-run` use `ckeletin-upstream/{{version}}` (lines 229, 279, 283). Sandbox proof: `git checkout origin/v0.2.0 -- .ckeletin/` → `fatal: invalid reference: origin/v0.2.0` (tags live in refs/tags/, not refs/remotes/<remote>/). `ckeletin-update-check-compatibility` already special-cases this correctly (lines 306-311: `git fetch ckeletin-upstream "{{version}}"; CHECKOUT_REF="FETCH_HEAD"`), so the three sibling recipes disagree. Additionally `git tag -l` shows only v0.1.0/v0.2.0 — no tags exist for framework versions 0.2.x at all, so the documented pin-to-version capability has never been exercisable. [dims: framework-dx]
- **Why it matters:** The spec doc explicitly promises tag-pinned updates; an agent following it gets a hard git error from `set -euo pipefail` mid-recipe (after fetch, before checkout — no state damage, but the documented capability silently never worked). Version pinning is the safety valve for a consumer that wants a known framework version rather than main-HEAD.
- **Recommendation:** Adopt check-compatibility's fetch+FETCH_HEAD resolution in `ckeletin-update` and `ckeletin-update-dry-run`, and start tagging framework releases (e.g. ckeletin-v0.2.16) so `version=` has real targets.
- **Verification:** confirmed

### Migration mechanism is documented in the design spec but entirely unimplemented

- **File:** `docs/specs/2026-04-14-framework-update-mechanism.md:199`
- **Ref:** Principle 7 — Single Source of Truth (doc vs implementation)
- **Evidence:** Spec doc Update Flow step 7: "if OLD_VERSION != NEW_VERSION, run all migration scripts in .ckeletin/migrations/ for versions between OLD and NEW in order"; §Version Compatibility and Migrations defines the `{version}.sh` convention; §Directory Layout lists `scripts/update.sh` and `scripts/migrate.sh`; §Migration Flow defines `just ckeletin-migrate prefix=...`. Reality: `grep -rn migrations .ckeletin/Justfile Justfile` → no matches; `git ls-files .ckeletin/migrations/` → empty (the dir is untracked); `.ckeletin/scripts/` contains only init.sh; no ckeletin-migrate recipe exists. [dims: framework-dx]
- **Why it matters:** The first upstream release that ships a breaking change with a migration script will assume consumers run it — they won't, because no code path ever reads .ckeletin/migrations/. A maintainer trusting this doc (the canonical update-mechanism spec) designs releases around a mechanism that does not exist. The doc also misdescribes today's two-tier `just check` gating (it documents only cargo-check + rollback).
- **Recommendation:** Either implement migration execution inside `ckeletin-update` (run scripts for versions in (OLD, NEW] before tier-1 verification) or amend the doc to mark migrations/migrate/update.sh as 'designed, not implemented' and document the implemented two-tier gate. Pick one truth.
- **Verification:** confirmed

### `just init` has no consumer/already-initialized guard — on a clean tree it silently runs `rm -rf .git` and resets CHANGELOG.md

- **File:** `.ckeletin/scripts/init.sh:97`
- **Ref:** Principle 9 — Automated Enforcement
- **Evidence:** init.sh's only safety check is the dirty-tree prompt (lines 16-31), which does not fire on a clean tree; it then unconditionally rewrites files, truncates CHANGELOG.md (lines 73-82), and runs `rm -rf .git; git init; git commit; git tag v0.0.0` (lines 97-101). Contrast: `ckeletin-update`, `-dry-run`, `-check-compatibility`, and all `conform*` recipes carry the upstream-slug guard (`grep -q "peiman/ckeletin-rust" Cargo.toml`) precisely because these recipes propagate. init.sh propagates to every consumer via .ckeletin/scripts/ and the root Justfile keeps the `init` recipe after init. [dims: framework-dx]
- **Why it matters:** In an established consumer repo, `just init name=oops` on a clean tree wipes the entire local git history (worktree content survives; local-only commits' history and tags do not) and destroys the project CHANGELOG, with zero prompt. This framework is explicitly built to be agent-driven, and agents enumerate just recipes — this is the one destructive recipe with no guard. The same asymmetry pattern (guards added to update/conform after consumers hit problems) predicts this will bite.
- **Recommendation:** Add the established fingerprint guard to init.sh: if the upstream slug is absent from the root Cargo.toml, refuse with "already initialized / consumer repo" (exit 1). Optionally also refuse when `git rev-list --count HEAD` exceeds the scaffold's history or a tag other than the scaffold's exists.
- **Verification:** confirmed

### VERSION-bump discipline is manual and has already been missed — content changed in .ckeletin/ at an unchanged VERSION

- **File:** `.ckeletin/VERSION`
- **Ref:** Principle 9 — Automated Enforcement; Principle 7 — SSOT
- **Evidence:** Commit 95b9d36 (2026-06-04, "chore(deps): bump toml 0.8->1 in conform + routine cargo patch bumps") modified `.ckeletin/conform/Cargo.toml` with no VERSION bump (git show --name-only confirms .ckeletin/VERSION untouched). `ckeletin-check-update` decides update_available purely by string equality of VERSION files (lines 198-202), and `ckeletin-update-dry-run` compares VERSIONs too. No CI job enforces '.ckeletin/ diff ⇒ VERSION bump' (reviewed ci.yml, spec-drift.yml, release.yml). [dims: framework-dx]
- **Why it matters:** Two different .ckeletin/ contents existed on main under the same version, so a consumer's `ckeletin-check-update json` reports `update_available:false` while content drifted — the trigger signal for the autonomous maintenance loop lies. Discipline has been excellent since PR #16 (every framework PR bumps VERSION), but it's convention, not enforcement, and the 95b9d36 miss proves convention leaks.
- **Recommendation:** Add a CI guard: if `git diff origin/main...HEAD --name-only -- .ckeletin/` is non-empty, require `.ckeletin/VERSION` to be among the changed files (and the new version to have a CHANGELOG heading).
- **Verification:** confirmed

### AGENTS.md predates the agent-facing surface: catalog command and the doctor/update/conform recipes are absent from the guide built for agents

- **File:** `AGENTS.md:47`
- **Ref:** CKSPEC-AGENT-004; CKSPEC-AGENT-005; CKSPEC-AGENT-006
- **Evidence:** grep -i 'catalog' AGENTS.md = zero hits, though `catalog` is the binary's third subcommand (crates/cli/src/root.rs:42-43 'Emit the machine-readable command catalog (CKSPEC-AGENT-006)') and the repo's newest conformance claim; `grep -n "doctor\|catalog\|ckeletin-update\|check-update" AGENTS.md README.md` returns nothing. The Commands table (AGENTS.md:47-58) lists none of: just conform, conform-refresh, conform-report, ckeletin-doctor, ckeletin-check-update, ckeletin-update, ckeletin-version, ckeletin-health, ckeletin-secrets, ckeletin-sbom, ckeletin-geiger, ckeletin-fuzz — all present in `just --list`. `conform`/`conform-refresh` appear only in Troubleshooting (:208). AGENTS.md last edit 2026-06-03; these landed 2026-06-04/05 (PRs #16–#26, incl. #23 'make the update/diagnostic surface agent-drivable'). [dims: framework-dx,docs]
- **Why it matters:** Everything the table DOES list works (verified), so this is drift, not error — but it is ironic drift: commands built explicitly to be agent-drivable (ckeletin-doctor json, catalog, the maintenance loop) are missing from the agent guide, so an agent must discover them via just --list and .ckeletin/Justfile source, exactly the trial-and-error AGENT-004's rationale says the guide exists to prevent. AGENTS.md is the declared entry point ('Read AGENTS.md first — it contains all project knowledge'), and CONFORMANCE.md:114 claims AGENT-004 met via command coverage.
- **Recommendation:** Add an 'Agent surface' subsection and Commands-table rows: `<binary> catalog --output json` for command discovery, conformance (just conform / conform-refresh / conform-report), framework maintenance (just ckeletin-doctor json, ckeletin-check-update json, ckeletin-update, ckeletin-health, ckeletin-version), and security scans (ckeletin-secrets / ckeletin-sbom / ckeletin-geiger / ckeletin-fuzz). Document the catalog subcommand next to the version-command callout (AGENTS.md:107) as the third worked example.
- **Verification:** confirmed

### 'Adding a New Command' walkthrough omits three required wiring steps — followed verbatim it does not compile

- **File:** `AGENTS.md:64`
- **Ref:** CKSPEC-AGENT-004
- **Evidence:** AGENTS.md:64-88 steps: (1) create crates/domain/src/mycommand.rs, (2) create crates/cli/src/mycommand.rs, (3) 'Add variant to Commands enum; Add match arm in run_inner() in main.rs', (4) integration test. Missing: (a) 'pub mod mycommand;' in crates/domain/src/lib.rs — the file currently contains only 'pub mod ping;', so step 1's file is never compiled; (b) 'mod mycommand;' in crates/cli/src/main.rs (mod decls at main.rs:4-7: 'mod catalog; mod ping; mod root; mod version;'); (c) an arm in subcommand_name() (main.rs:72-78), whose match is deliberately exhaustive, so step 3 alone yields error[E0004] non-exhaustive match. [dims: docs]
- **Why it matters:** Docs are the contract for agents here, and the recipe's promise is that following it produces a working command. Omission (c) at least fails loudly by design (and the convention is documented separately at AGENTS.md:124), but (a) and (b) are plain gaps: the domain module silently never compiles and the handler module is unresolved. An agent recovers via compiler errors, burning tokens the guide exists to save (the spec's own AGENT-004 rationale).
- **Recommendation:** Add the three wiring steps: declare 'pub mod mycommand;' in crates/domain/src/lib.rs (step 1), 'mod mycommand;' in crates/cli/src/main.rs (step 2), and in step 3 note the required subcommand_name() arm with a pointer to the exhaustive-match convention. (The catalog needs no step — it derives from the clap tree, which is worth saying explicitly as a non-step.)
- **Verification:** confirmed

### Release tag and CARGO_PKG_VERSION are never reconciled, and the v* tag namespace is shared between framework pins and binary releases — repo history already shows tag v0.2.0 built from package version 0.1.0

- **File:** `.github/workflows/release.yml:14`
- **Ref:** CKSPEC-OUT-006
- **Evidence:** release.yml triggers `on: push: tags: ["v*"]` and ships `target/release/ckeletin-rust` with `gh release create "${GITHUB_REF_NAME}"` — no step compares the tag to the package version. Tags v0.1.0 (2026-04-13) and v0.2.0 (2026-04-15) already exist; `git show v0.2.0:Cargo.toml` shows `version = "0.1.0"`, and workspace.package version is still 0.1.0 today, so the binary self-reports `ckeletin-rust 0.1.0, commit 91480da` (verified by running `--version`). Simultaneously, docs/specs/2026-04-14-framework-update-mechanism.md:197 documents v* tags as the FRAMEWORK pin namespace ('a branch name or tag like `v0.2.0`') consumed by `ckeletin-update version=` (.ckeletin/Justfile:213), and .ckeletin/VERSION is at 0.2.16. `gh release list` and `gh run list --workflow=release.yml` both return empty, so this is latent, not shipped.
- **Why it matters:** The next tag push — most plausibly a framework release tag like v0.2.17 or v0.3.0 — fires release.yml and publishes a GitHub release of the demo app binary whose `version` subcommand (the CKSPEC-OUT-006 build-identity showcase) reports 0.1.0 under a release named v0.2.17. The scaffold's flagship 'honest build identity' guarantee is contradicted by its own release pipeline, and two unrelated release processes silently share one trigger namespace.
- **Recommendation:** Add a guard step in the publish job: assert `"v$(cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | select(.name=="cli") | .version')" == "${GITHUB_REF_NAME}"` and fail otherwise. Separate the namespaces: tag framework releases as e.g. `framework-v*` (and fix the ckeletin-update ref resolution accordingly) or scope release.yml's trigger so framework tags don't publish app binaries.
- **Verification:** confirmed

## Severity: low

### First-run notice and config default name a log file that never exists (daily roller appends a date suffix)

- **File:** `crates/cli/src/main.rs:114`
- **Ref:** CKSPEC-OUT-004
- **Evidence:** Notice printed: `note: writing an audit log to <HOME>/.config/ckeletin-rust/logs/app.log ...` but the file actually created (because logging.rs:134 uses `tracing_appender::rolling::daily`) is `<HOME>/.config/ckeletin-rust/logs/app.log.2026-06-09`. Verified with a fresh HOME: `find` shows only `app.log.2026-06-09`; no `app.log` exists. The config default `log_file_path = "logs/app.log"` (config.rs:50) implies the same nonexistent name. [dims: correctness]
- **Why it matters:** The one message designed to tell users/agents where their audit evidence lives points at a path that never exists. An agent that runs `cat` on the printed path fails; a human tails the wrong file. Small, but it is the discoverability seam for the audit feature.
- **Recommendation:** Print the log directory instead of the file (`audit_path.parent()`), or mention the date-suffix convention in the notice, or switch to `Rotation::NEVER` so the configured filename is the real filename.
- **Verification:** confirmed

### --config pointing at a directory silently succeeds with defaults, defeating the explicit-config guard

- **File:** `.ckeletin/crate/src/config.rs:90`
- **Ref:** —
- **Evidence:** The guard is `if !std::path::Path::new(path).exists() { return Err(...) }` with the comment 'Explicit --config: file MUST exist. Silent fallback is misleading.' A directory passes `.exists()`, and figment's `Toml::file` on a directory contributes nothing. Verified: `ckeletin-rust --config /tmp ping` -> `Pong! ckeletin-rust is alive`, exit 0, defaults applied. (An unreadable file does error correctly.) [dims: correctness]
- **Why it matters:** Exactly the silent-fallback failure mode the guard was written to prevent: a typo that resolves to a directory (e.g. `--config ./conf` where conf/ is a dir) makes the user believe their config is active while defaults run. Framework file — propagates downstream.
- **Recommendation:** Change the check to `Path::new(path).is_file()` and adjust the error message to 'config file not found or not a file'.
- **Verification:** confirmed

### Empty log_file_path drops the audit log in the wrong place with a misleading notice

- **File:** `.ckeletin/crate/src/logging.rs:125`
- **Ref:** —
- **Evidence:** With cwd config.toml `log_file_path = ""`: resolve_audit_path joins "" onto the app base giving `.../.config/ckeletin-rust/` (trailing); in prepare_file_appender, `Path::file_name()` ignores the trailing empty segment and returns "ckeletin-rust" with parent ".config". Verified on a fresh HOME: notice says `writing an audit log to .../.config/ckeletin-rust/` (a directory), and the file actually created is `HOME/.config/ckeletin-rust.2026-06-09` — a sibling of all apps' config dirs; the per-app dir is never created. [dims: correctness]
- **Why it matters:** A plausible misconfiguration (empty string to 'disable' the path) silently scatters audit files outside the app's directory instead of erroring or falling back to the default. String/path edge in framework code that downstream repos inherit.
- **Recommendation:** In Config::load or resolve_audit_path, treat an empty `log_file_path` as invalid: either error explicitly or fall back to the default `logs/app.log`. Guard `file_name().unwrap_or_default()` being empty in prepare_file_appender with an io::Error.
- **Verification:** confirmed

### Error envelope omits `data`; success envelope omits `error` — spec asks for explicit null fields

- **File:** `.ckeletin/crate/src/output.rs:20`
- **Ref:** CKSPEC-OUT-003
- **Evidence:** `#[serde(skip_serializing_if = "Option::is_none")]` on both `data` and `error` (output.rs:20-23). Verified output: `ckeletin-rust --output json --config /nonexistent.toml ping` → {"status": "error", "command": "ping", "error": "config file not found: /nonexistent.toml"} — no `data` key. CKSPEC-OUT-003 (SHOULD): envelope 'with at minimum: status…, command identifier, data payload (null on error), and error details (null on success)'. [dims: correctness,errors]
- **Why it matters:** Absent-vs-null is invisible to jq (`.data` → null either way) but differs for strict-schema consumers and for cross-implementation envelope parity (an agent driving both ckeletin-go and ckeletin-rust may see two shapes). SHOULD-level, so calibrated low — but it's a one-line divergence from the spec's literal envelope description.
- **Recommendation:** Drop the `skip_serializing_if` attributes so `data: null` / `error: null` serialize explicitly (confirming against ckeletin-go's envelope output for byte-shape parity), and update the envelope tests; or record the omission as a deliberate deviation in conformance-mapping.toml's OUT-003 evidence.
- **Verification:** confirmed

### owo-colors is a declared but completely unused dependency of the cli crate

- **File:** `crates/cli/Cargo.toml:18`
- **Ref:** Principle 3 — Lean Iteration
- **Evidence:** crates/cli/Cargo.toml:18 `owo-colors = { workspace = true }`; cargo tree -p cli --depth 1 confirms it as a direct dependency (owo-colors v4.3.0); `grep -rni "owo" crates .ckeletin --include="*.rs"` returns zero matches. Nothing in the pipeline (clippy, deny, conform) flags unused dependencies. [dims: architecture]
- **Why it matters:** Dead supply-chain surface in the reference implementation that every scaffold consumer inherits at init; it also quietly contradicts the repo's minimal-deps story (cargo-deny audits a crate the binary never uses).
- **Recommendation:** Remove owo-colors from crates/cli/Cargo.toml and [workspace.dependencies] (or actually use it in human-mode rendering); consider adding cargo-machete/cargo-udeps to `just check` or CI to catch future dead deps.
- **Verification:** confirmed

### Framework crate manifest version (0.2.0) drifted 16 patch releases behind .ckeletin/VERSION (0.2.16)

- **File:** `.ckeletin/crate/Cargo.toml:3`
- **Ref:** Principle 7 — SSOT
- **Evidence:** .ckeletin/crate/Cargo.toml:3 `version = "0.2.0"` and .ckeletin/conform/Cargo.toml:3 `version = "0.2.0"`, while .ckeletin/VERSION contains `0.2.16` — the value all Justfile recipes (ckeletin-version, doctor JSON, update flow, ckeletin-health/check-update) actually consume. framework_purity.rs guards name leakage but nothing checks version agreement. [dims: architecture,manifesto]
- **Why it matters:** Two parallel version declarations for the same artifact, one stale by 16 releases. `cargo metadata` or any tooling reading the crate version reports 0.2.0 for a 0.2.16 framework, and this propagates to every downstream repo via ckeletin-update — exactly the duplicated-value drift the repo's own SSOT discipline forbids.
- **Recommendation:** Either bump the crate versions in the release flow alongside .ckeletin/VERSION (with a framework_purity-style invariant test asserting the two agree), or pin them at a sentinel (e.g. 0.0.0) with a comment declaring .ckeletin/VERSION the sole version source.
- **Verification:** confirmed

### Entry point has crept past 'bootstrap only': stale ~20-line claim in AGENTS.md, and first-run notice logic in main.rs is never audit-logged

- **File:** `crates/cli/src/main.rs:109`
- **Ref:** CKSPEC-ARCH-006; CKSPEC-OUT-004
- **Evidence:** AGENTS.md:33 says 'main.rs # Bootstrap only (~20 lines)'; main.rs is 149 lines (wc -l), and CONFORMANCE.md:44 already corrected the figure once — to 102, also now stale. main.rs:109-120 contains the first-run audit-notice feature (directory-existence probing + raw `eprintln!`), emitted BEFORE logging::init at line 133, so this user-facing output bypasses the shadow log despite AGENTS.md:12's 'every output logged to an audit file'. [dims: architecture]
- **Why it matters:** The entry point is accreting testable feature logic in the one place the spec says should be minimal, and the primary agent guide's description no longer matches reality — agents trusting AGENTS.md get a wrong mental model. The unaudited eprintln is a small, defensible chicken-and-egg exception (the notice is about creating the log dir) but is undocumented as such.
- **Recommendation:** Fix AGENTS.md:33 to describe main.rs honestly (drop the line count or state the real shape); extract the first-run-notice predicate into infrastructure::logging (unit-testable, returns Option<String>) leaving main.rs to print it; add a comment documenting why this one status line precedes audit-log initialization.
- **Verification:** confirmed

### ENF-005 completeness check is one-directional: unknown extra mapping entries pass silently and inflate totals

- **File:** `.ckeletin/conform/src/main.rs:168`
- **Ref:** CKSPEC-ENF-005
- **Evidence:** `find_unmapped` (main.rs:168-177) only filters spec IDs absent from the mapping. No code checks the reverse (mapping keys ⊆ spec IDs), and the summary uses `total: mapping.requirements.len()` (main.rs:281, 495). A mapping entry for a typo'd or spec-removed ID (e.g. 'CKSPEC-FAKE-001' with status=met and any anchor) would pass all gates and publish a '41/41 met' report against a 40-requirement spec. [dims: spec-conformance]
- **Why it matters:** The letter of CKSPEC-ENF-005 ('mapping covers every requirement') is met — verified by the unit test find_unmapped_flags_a_requirement_missing_from_the_mapping and the live gate. But when the spec *removes or renames* a requirement, the stale mapping entry survives and the published summary's total drifts from the spec's requirement count, which is the same false-confidence class ENF-005 targets. Low because spec requirements have so far only been added, and the spec-repo aggregator presumably keys off its own ID list.
- **Recommendation:** Add the reverse check: fail (or signal) when the mapping contains an ID not present in the vendored spec snapshot, and add a unit test mirroring find_unmapped_* for the reverse direction.
- **Verification:** confirmed

### ci.yml claims tool-version pins are 'kept fresh by Dependabot'; dependabot.yml states the opposite

- **File:** `.github/workflows/ci.yml:21`
- **Ref:** Principle 1 — Truth-Seeking
- **Evidence:** ci.yml:18-21: 'Tool versions are PINNED... Bumping these is a deliberate, reviewable edit (kept fresh by Dependabot).' But .github/dependabot.yml explicitly notes: 'the `cargo install <tool> --version X` pins in ci.yml are run-commands, not action refs, so they are bumped by hand'. Dependabot's github-actions ecosystem only updates `uses:` refs, so just 1.50.0, cargo-deny 0.19.4, cargo-llvm-cov 0.8.5, cargo-cyclonedx 0.5.9 and gitleaks 8.30.1 have no freshness mechanism. [dims: manifesto]
- **Why it matters:** Two comments disagree about the freshness mechanism for the same pins; a maintainer trusting the ci.yml comment will assume staleness is impossible and the pins will quietly age.
- **Recommendation:** Fix the ci.yml comment to match dependabot.yml ('action refs by Dependabot; run-command tool pins by hand'), or move tool installs to a Dependabot-visible mechanism (e.g. taiki-e/install-action pinned via uses:).
- **Verification:** confirmed

### Shipped plan kept with 52/52 unchecked checkboxes and an active agent-execution header; companion design doc still says 'Status: Design approved'

- **File:** `docs/plans/2026-04-14-framework-update-mechanism.md:3`
- **Ref:** Principle 5 — Platforms, Not Features
- **Evidence:** docs/plans/2026-04-14-framework-update-mechanism.md:3: '**For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development ... to implement this plan task-by-task.' grep -c '\- \[ \]' → 52 unchecked, 0 checked — yet the mechanism shipped long ago (.ckeletin/ exists, framework at v0.2.16). docs/specs/2026-04-14-framework-update-mechanism.md:4: '**Status:** Design approved, revised after manifesto review' — never updated to implemented, and its content (e.g. `binary_name := "myproject"`, init steps) predates the shipped flow. [dims: manifesto]
- **Why it matters:** In an agent-driven repo this is live scaffolding rot: an agent pointed at docs/plans/ sees an explicitly executable plan that appears 0% done and could re-execute or trust stale design details that the implementation has since superseded (e.g. the update flow's tier-2 `just check` gate, conform no-op guards).
- **Recommendation:** Add a status banner to both docs ('IMPLEMENTED as of <commit/version> — historical record; do not execute') or check the boxes / move them to an archive folder.
- **Verification:** confirmed

### Serialization-failure branch of Output::success/message ('JSON mode with non-serializable data') is untested

- **File:** `.ckeletin/crate/src/output.rs:85`
- **Ref:** CKSPEC-OUT-003
- **Evidence:** `let envelope = Envelope::success(command, data).map_err(io::Error::other)?;` — the Err arm is never exercised; the coverage table shows output.rs at 98.79% lines with 3 missed lines, which are these map_err/error-propagation paths. Envelope::success returns Result<Self, serde_json::Error> and no test feeds a failing Serialize impl. [dims: tests]
- **Why it matters:** The framework promises any `Serialize + Display` type renders safely in JSON mode; what a consumer actually sees when serialization fails (io::Error bubbling into the top-level error envelope vs. partial output) is unverified behavior in vendored framework code that propagates downstream.
- **Recommendation:** Add a unit test with a test double whose Serialize impl returns Err (e.g. `serializer.serialize_map(...)` with a non-string key, or a custom `impl Serialize` returning `Err(ser::Error::custom(..))`), asserting Output::success returns Err and writes nothing to the data writer.
- **Verification:** confirmed

### Integration JSON assertions are pretty-print-coupled substrings rather than parsed JSON

- **File:** `crates/cli/tests/cli.rs:47`
- **Ref:** CKSPEC-OUT-003
- **Evidence:** Tests assert substrings like `predicate::str::contains("\"command\": \"version\"")` and `contains("\"name\": \"ping\"")` — these encode serde_json's to_writer_pretty spacing (`"key": value` with one space) and can false-match (e.g. catalog_json_lists_every_subcommand's `"name": "ping"` would also match a flag or description containing that text). The framework's own unit tests show the better pattern: `let envelope: Envelope = serde_json::from_slice(&buf).unwrap(); assert_eq!(envelope.command, "ping")` (output.rs tests). [dims: tests]
- **Why it matters:** If the envelope writer ever switches to compact serialization (a non-breaking change for machine consumers), every JSON integration test fails at once for the wrong reason; and substring matching can pass against structurally wrong output. As exemplar code, consumers copy this weaker pattern instead of the parse-then-assert pattern the same repo demonstrates in unit tests.
- **Recommendation:** In cli.rs, capture stdout, `serde_json::from_str` it, and assert on the parsed structure (status, command, data.message) — at least for the envelope-shape tests; keep substring predicates for human-mode output only.
- **Verification:** confirmed

### Audit log records output.success for runs whose output never reached the user (shadow log written before the write attempt)

- **File:** `.ckeletin/crate/src/output.rs:81`
- **Ref:** CKSPEC-OUT-004
- **Evidence:** `tracing::debug!(command = command, data = %data, "output.success");` (output.rs:81) executes before the `writeln!`/`to_writer_pretty` calls (same pattern in `message` at :108). Empirically: in the EPIPE experiment the run FAILED (exit=1, user saw `Error: Broken pipe`) yet the audit log gained `{"fields":{"message":"output.success","command":"ping","data":"Pong! ckeletin-rust is alive"}}` at 20:16:30 — recording as delivered output that the user never received, with no compensating error record (see the dropped output.error finding). [dims: errors]
- **Why it matters:** CKSPEC-OUT-004 says the audit log MUST contain 'at least the data that was rendered to the user'. Logging intent before outcome means on any write failure the audit trail asserts something false — it shows a successful pong for a run that exited 1. Combined with the dropped output.error event, the audit trail for a failed run is indistinguishable from a successful one. Principle 1 — Truth-Seeking: the audit log is supposed to be the evidence of what happened.
- **Recommendation:** Either emit the shadow event after the write succeeds, or keep the pre-write event but rename it to reflect intent (e.g. `output.render_attempt`) and emit a paired outcome event (success/error) after the write — the failure half of which requires the guard-lifetime fix above to actually land in the file.
- **Verification:** downgraded

### `just test` pipes nextest's stderr to /dev/null and falls back to a full cargo-test rerun on ANY nextest failure — can mask flaky tests and discards the primary runner's failure output

- **File:** `Justfile:14`
- **Ref:** —
- **Evidence:** `test:\n    cargo nextest run --workspace 2>/dev/null || cargo test --workspace` (Justfile:13-14). cargo-nextest writes its run/progress/failure output to stderr, so on a machine WITH nextest installed all of it is discarded; a real test failure under nextest triggers a from-scratch rerun of the entire suite under a different runner, and if the failing test is flaky and passes on the rerun, `just test` (and therefore `just check`, the single gateway, CKSPEC-ENF-001) exits 0. nextest-specific failures (config errors, leaked-process detection) are likewise silently converted into a cargo-test rerun. (Verified on this machine nextest is absent, so the fallback path ran with visible output.) [dims: errors]
- **Why it matters:** The `2>/dev/null` exists to hide cargo's 'no such command' error when nextest is missing, but it is far broader than that: it swallows the primary test runner's entire diagnostic stream and turns 'nextest failed' into 'try again with another runner' — a retry that can flip a red gate green. This recipe is the test gateway propagated to every downstream repo.
- **Recommendation:** Branch on tool presence instead of exit code: `if command -v cargo-nextest >/dev/null 2>&1; then cargo nextest run --workspace; else cargo test --workspace; fi` — nextest output stays visible and a test failure fails the gate exactly once.
- **Verification:** downgraded

### Post-init audit write failures are silent: lossy non-blocking appender drops lines and worker-thread I/O errors are never surfaced

- **File:** `.ckeletin/crate/src/logging.rs:135`
- **Ref:** CKSPEC-OUT-004
- **Evidence:** `Ok(tracing_appender::non_blocking(file_appender))` (logging.rs:135) uses tracing-appender defaults: lossy=true (lines silently dropped when the 128k buffer fills) and a worker thread whose write errors (disk full, file deleted mid-run, permission flip) have no channel back to the user. Init-time failure IS loud — verified: `CKELETIN_LOG_FILE_PATH=/dev/null/impossible/audit.log ckeletin-rust ping` → `Error: Not a directory (os error 20)`, exit 1 — but nothing after init can report. [dims: errors]
- **Why it matters:** For short-lived CLI runs the loud init check covers the common cases (missing dir, bad path), so exposure is small — but a long-running downstream command that loses the audit file mid-run keeps reporting success while the MUST-level audit stream silently stops. The user observes nothing; a best-effort warning at shutdown would be honest.
- **Recommendation:** Low priority: consider `NonBlockingBuilder::default().lossy(false)` for the audit stream, and/or check the appender's error counter when LogGuard drops and emit a stderr warning if lines were dropped.
- **Verification:** confirmed

### process::run_capture discards child stderr — failures report only an exit code, losing the child's diagnostic

- **File:** `.ckeletin/crate/src/process.rs:18`
- **Ref:** —
- **Evidence:** `.stderr(std::process::Stdio::null())` (process.rs:18); on failure: `io::Error::other(format!("{} exited with status {}", cmd, output.status.code().unwrap_or(-1)))` (process.rs:22-26). A consumer command built on this helper surfaces e.g. `Error: git exited with status 128` with the actual reason (`fatal: not a git repository…`) discarded; signal-terminated children all report as -1. [dims: errors]
- **Why it matters:** This is the framework's sanctioned plug point for consumer commands that shell out (AGENTS.md directs CLI-layer discovery through it). The error envelope downstream users and agents see will routinely say only 'exited with status N' — the user should observe the child's stderr, which is the diagnostic. Exit code is correct and the failure is loud, hence low rather than medium.
- **Recommendation:** Capture stderr (`.output()` already does when not nulled) and include a trimmed copy in the error message, e.g. `"{cmd} exited with status {code}: {stderr}"`; distinguish signal termination from code -1.
- **Verification:** confirmed

### init.sh destroys git history before verifying `git commit` can succeed, and its dirty-tree preflight misses staged changes

- **File:** `.ckeletin/scripts/init.sh:96`
- **Ref:** —
- **Evidence:** Preflight uses `git diff --quiet` (init.sh:16) — which ignores staged-but-uncommitted changes (`git diff --cached` would catch them) and says nothing about committed-but-unpushed work, though `rm -rf .git` (init.sh:97) destroys all local history. Steps 96-101 then run `rm -rf .git; git init; git add -A; git commit …` with no preflight that `user.name`/`user.email` are configured; on a fresh machine `git commit` fails ('Please tell me who you are') AFTER the old history is gone, aborting via set -e before the tag and 'Done!' message. [dims: errors]
- **Why it matters:** Both failure modes are at least loud (set -e), and file contents survive in the working tree, so data loss is limited to history — which init intends to discard anyway. But the partial state (renamed project, no initial commit, no v0.0.0 tag) ships with no recovery guidance, and a user with staged work gets no warning at all before the point of no return.
- **Recommendation:** Before `rm -rf .git`: extend the dirty check to `git diff --quiet && git diff --cached --quiet`, and verify `git config user.name` / `user.email` resolve (or pass `-c user.name=… -c user.email=…` fallbacks to the commit); print recovery instructions if the commit step fails.
- **Verification:** confirmed

### Conform generator: failing checks run with stdout/stderr nulled, and date resolution panics via expect

- **File:** `.ckeletin/conform/src/main.rs:566`
- **Ref:** CKSPEC-ENF-005
- **Evidence:** `run_check` pipes both streams to null (main.rs:568-571), so a failed conformance check prints only `  CKSPEC-…  <cmd> ... FAIL` with the command's own diagnostic discarded; `chrono_free_date()` uses `.expect("date command failed")` (main.rs:602) — a panic, not a typed error, if `date` can't spawn. By contrast every file/parse error path in the same binary is exemplary: explicit eprintln + exit(1) with remediation hints (e.g. main.rs:143-145 tells you to run `just conform-refresh`). [dims: errors]
- **Why it matters:** Maintainer-only tooling, all failures are loud and gate CI correctly (exit 1 verified by code path), so this is polish: the operator just has to re-run the printed command by hand to see why it failed, and the `date` panic is near-unreachable on POSIX. Calibrated low accordingly.
- **Recommendation:** Capture check output and print it (or its tail) on FAIL; replace the `expect` with a fallback to "unknown" matching the project's own honest-degradation idiom.
- **Verification:** confirmed

### ci.yml has no permissions block — default GITHUB_TOKEN scope while executing untrusted PR code

- **File:** `.github/workflows/ci.yml:1`
- **Ref:** Principle 9 — Automated Enforcement
- **Evidence:** ci.yml contains no `permissions:` key at workflow or job level (full file read; only `name`, `on`, `env`, `jobs`). By contrast release.yml:19-20 sets `permissions: contents: write` and spec-drift.yml:19-21 sets `contents: read / issues: write` — so the repo knows the pattern and ci.yml is the inconsistent one. The CI `check` job runs `just check` on pull_request, which compiles and executes arbitrary PR code (build.rs, proc macros, tests), and actions/checkout persists the GITHUB_TOKEN in .git/config by default (persist-credentials defaults to true). [dims: security]
- **Why it matters:** Without an explicit block, the token scope falls back to repository/organization defaults, which vary — and downstream clones of this template inherit whatever the cloner's org default is (older orgs default to read-write). Fork PR tokens are read-only, but same-repo branch PRs get the full default token sitting on disk while untrusted-ish build scripts run. Least privilege should be declared, not inherited.
- **Recommendation:** Add top-level `permissions: contents: read` to ci.yml (no CI job needs more). Optionally add `persist-credentials: false` to checkout steps in jobs that never push.
- **Verification:** downgraded

### `just init` interpolates {{name}} unquoted — shell injection bypasses init.sh's name validation

- **File:** `Justfile:34`
- **Ref:** Principle 9 — Automated Enforcement
- **Evidence:** Justfile:33-34: `init name:` / `    .ckeletin/scripts/init.sh {{name}}`. Verified: `just -n init 'foo; echo INJECTED'` renders the recipe line as `.ckeletin/scripts/init.sh foo; echo INJECTED` — the second command would execute, and init.sh's regex guard at .ckeletin/scripts/init.sh:7 (`^[a-z][a-z0-9-]*$`) never sees the payload because the shell has already split it. [dims: security]
- **Why it matters:** Locally this is self-injection (the invoker already has a shell), so it is not a privilege boundary today — hence low. But this repo's stated endgame is agent-driven automation (workhorse), where a project name may arrive from a config file, issue title, or LLM output; the validation that looks load-bearing in init.sh is silently bypassable one layer up. It also teaches downstream template users an unsafe just pattern.
- **Recommendation:** Quote the interpolation: `.ckeletin/scripts/init.sh '{{name}}'` (or declare the recipe with a shebang and pass "$1" via positional args). One-character-pair fix; init.sh's regex then becomes the effective gate.
- **Verification:** confirmed

### deny.toml does not deny yanked crates (cargo-deny default is warn)

- **File:** `deny.toml:24`
- **Ref:** CKSPEC-ENF-001
- **Evidence:** deny.toml:24-25 is `[advisories]` / `ignore = []` with no `yanked` key. cargo-deny docs (advisories cfg, v2 — confirmed against embarkstudios.github.io/cargo-deny): "`warn` (default) - Prints a warning with the crate name and version that was yanked, but does not fail the check." Vulnerabilities and unmaintained advisories do error by default, so those are covered. [dims: security]
- **Why it matters:** Crates are frequently yanked for soundness or security reasons; with the default, a yanked version frozen in Cargo.lock sails through `just check`, the CI check job, and the weekly advisories heartbeat with only a log-line warning nobody reads. For a scaffold whose advisories heartbeat exists specifically to catch silent rot in a frozen lockfile (ci.yml:9-11), this is the matching gap.
- **Recommendation:** Add `yanked = "deny"` under `[advisories]` in deny.toml.
- **Verification:** confirmed

### release.yml hardcodes target/release/ckeletin-rust — first release in a consumer repo fails (or worse, ships a stale binary)

- **File:** `.github/workflows/release.yml:64`
- **Ref:** Principle 7 — Single Source of Truth
- **Evidence:** release.yml:64 uploads the literal path `target/release/ckeletin-rust`. init.sh renames the binary (crates/cli/Cargo.toml `name`, init.sh:45) and rewrites the Justfile's `binary_name` (init.sh:52), but never touches .github/workflows/. Unlike init-smoke and spec-drift, release.yml has no upstream-only guard, so it runs on any `v*` tag in a derived repo: the conform gate no-ops there (exit 0, .ckeletin/Justfile:356-358), the build succeeds under the new binary name, then `gh release create` references a path that no longer exists. [dims: security]
- **Why it matters:** Primarily a correctness defect (release breaks at first tag downstream), but it is an artifact-integrity edge: if a stale `ckeletin-rust` binary from a pre-init build ever exists in target/release (e.g. cache or committed local experimentation), the workflow would attach the wrong binary to the release.
- **Recommendation:** Have init.sh rewrite the path in release.yml (it already rewrites Justfile's binary_name), or derive the artifact path from the Justfile SSOT (e.g. `just build && cp target/release/$(just --evaluate binary_name) dist/`), and name the asset with version + target triple.
- **Verification:** confirmed

### Shipped framework version 0.2.15 was erased from the CHANGELOG — PR #25 retitled its heading to 0.2.16

- **File:** `.ckeletin/CHANGELOG.md:3`
- **Ref:** —
- **Evidence:** PR #24 (7b5b597) shipped VERSION 0.2.15 with its own `## [0.2.15] - 2026-06-05` heading (verified via `git show 7b5b597:.ckeletin/CHANGELOG.md`). PR #25 (4b5dbe6) diff shows `-## [0.2.15] - 2026-06-05` / `+## [0.2.16] - 2026-06-05` — the conform-fix entry was folded under 0.2.16 and no [0.2.15] heading exists in the current file. [dims: framework-dx]
- **Why it matters:** 0.2.15 was a real, consumable state on main (consumers update from main-HEAD at arbitrary times). A consumer that updated at 0.2.15 has a version string with no changelog anchor; release headings should be immutable once a VERSION value has been committed to main. Minor today, but it undermines the changelog as the agent-readable record of what changed between framework versions.
- **Recommendation:** Keep per-version headings immutable: PR #25 should have added a new [0.2.16] section above [0.2.15]. Restore the [0.2.15] heading, and fold this rule into the CI guard from the VERSION-discipline finding.
- **Verification:** confirmed

### ckeletin-check-update lacks the upstream self-guard its three siblings have — on upstream it instructs adding a remote pointing at itself

- **File:** `.ckeletin/Justfile:186`
- **Ref:** —
- **Evidence:** Ran `just ckeletin-check-update json` in this (upstream) repo: `{"error":"no_upstream_remote","hint":"git remote add ckeletin-upstream https://github.com/peiman/ckeletin-rust.git"}` — the hint tells the operator/agent of the upstream repo to add a remote to itself. `ckeletin-update`, `-dry-run`, and `-check-compatibility` all start with the slug guard and are covered by update_guard.rs (which loops over exactly those three recipes, lines 84-88); check-update is in neither. [dims: framework-dx]
- **Why it matters:** An autonomous loop polling `ckeletin-check-update json` (its documented purpose) on the upstream repo follows the hint, adds a self-remote, and thereafter gets a permanently 'up to date' answer — harmless but wrong, and inconsistent with the deliberate guard pattern of every other propagating recipe.
- **Recommendation:** Add the same slug short-circuit (e.g. emit `{"error":"upstream_repo"}` / 'framework update does not apply here', exit 0) and add the recipe to the update_guard.rs loop.
- **Verification:** confirmed

### ckeletin-doctor JSON omits rustfmt/clippy component presence that text mode reports and `just check` hard-requires

- **File:** `.ckeletin/Justfile:44`
- **Ref:** —
- **Evidence:** JSON branch (lines 40-48) emits booleans for cargo-deny, cargo-llvm-cov, cargo-nextest, just, gitleaks, cargo-cyclonedx, grype, cargo-geiger, cargo-bolero — verified by running `just ckeletin-doctor json` (valid JSON, parsed). The rustfmt/clippy component checks exist only in the text branch (lines 68-69). `just check` → ckeletin-fmt-check/ckeletin-clippy fail hard without those components. [dims: framework-dx]
- **Why it matters:** The 0.2.14 changelog sells doctor json as "machine preflight for 'is this environment ready'", but an agent parsing it cannot detect the two missing components most likely to break `just check` on a fresh toolchain (rustup installs them by default profile, but minimal-profile CI images do not).
- **Recommendation:** Add `"rustfmt":<bool>,"clippy":<bool>` to the JSON tools object (via `cargo fmt --version` / `cargo clippy --version` probes, same as text mode) and assert them in doctor.rs's doctor_json_is_machine_readable test.
- **Verification:** confirmed

### Stale comments in vendored framework contradict current init.sh and the update-guard fingerprint

- **File:** `.ckeletin/crate/tests/init_smoke.rs:16`
- **Ref:** —
- **Evidence:** init_smoke.rs lines 16-18: "init.sh's own verification compiles lib + bin targets but NOT test targets" (repeated at lines 136-139, and in .github/workflows/ci.yml line ~127). But init.sh line 88 runs `cargo check --workspace --all-targets -q` with a comment explicitly saying it covers tests. Separately, update_guard.rs lines 9-11 say the upstream fingerprint is "crates/cli/Cargo.toml's `name`", while both the Justfile guard (line 216: `grep -q "peiman/ckeletin-rust" Cargo.toml`) and the test's own skip check (lines 58-60) use the root manifest's repository slug. [dims: framework-dx]
- **Why it matters:** These files propagate verbatim to every consumer; the comments are the rationale future maintainers and agents read before changing guard/verification logic. A maintainer trusting the init_smoke comment might weaken init.sh's check as 'redundant with the smoke test', and the wrong fingerprint description invites an inconsistent guard implementation in the next recipe.
- **Recommendation:** Update the three comment sites to match reality: init.sh checks all targets (init_smoke remains valuable because it RUNS the suite and exercises the rename end-to-end); the fingerprint is the workspace repository slug in the root Cargo.toml.
- **Verification:** confirmed

### lefthook commit-msg gate matches ANY line of the message, not the subject — a non-conventional subject passes if any body line matches

- **File:** `lefthook.yml:23`
- **Ref:** CKSPEC-ENF-001
- **Evidence:** The hook does `MSG=$(cat {1})` then `echo "$MSG" | grep -qE "^(feat|fix|...)..."`. grep matches per-line, so the anchor `^` applies to every line. Demonstrated: `printf 'WIP random subject\n\nfix: sneaky body line\n' | grep -qE "^(feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert)(\\(.+\\))?: .+"` exits 0 (prints REGEX-PASSES-ON-BODY-LINE). The file's own header (line 1) claims 'automated enforcement (CKSPEC-ENF-001)'.
- **Why it matters:** The conventional-commit gate that CLAUDE.md says 'Lefthook enforces' is bypassable by any commit whose body mentions e.g. 'fix: ...' — common in revert/squash bodies. There is also no CI-side backstop for message format, so the only enforcement rung is this hole-y local hook.
- **Recommendation:** Validate only the first line: `head -n1 "{1}" | grep -qE '^(feat|fix|...)...'`. Optionally add `skip: [merge, rebase]` so merge commits don't force --no-verify (which disables all hooks at once).
- **Verification:** confirmed

### cargo-geiger is installed unpinned, contradicting ci.yml's own 'Tool versions are PINNED, not floating' policy stated 40 lines above

- **File:** `.github/workflows/ci.yml:61`
- **Ref:** Principle 7 — SSOT
- **Evidence:** ci.yml:18-21 comment: 'Tool versions are PINNED, not floating: `cargo install <tool> --locked` only pins the tool's own dependencies, NOT the tool version...'. Every sibling install pins (just 1.50.0, cargo-deny 0.19.4, cargo-llvm-cov 0.8.5, cargo-cyclonedx 0.5.9, gitleaks 8.30.1), but line 61 is `run: cargo install cargo-geiger --locked` — no --version.
- **Why it matters:** Same failure mode the comment warns about: a breaking cargo-geiger release changes the advisory job's behavior with no change in this repo. Impact is contained (the job is schedule-only with continue-on-error: true), but the file's stated policy is violated by its own contents — the prose-vs-machine drift pattern every other reviewer found, in one more spot.
- **Recommendation:** Pin it: `cargo install cargo-geiger --version <x.y.z> --locked`, matching the policy and the dependabot story already flagged for the other pins.
- **Verification:** confirmed

### README never mentions `just init` — the template's primary workflow is absent from its front door, and the architecture diagram repeats the pre-.ckeletin layout

- **File:** `README.md:16`
- **Ref:** Principle 7 — SSOT
- **Evidence:** `grep -in init README.md` returns nothing. The Quick Start (lines 18-24) only clones and runs the scaffold itself (`cargo run -p cli -- ping`); scaffolding a new project — the repo's stated purpose ('Rust CLI scaffold', line 3) via `just init name` (Justfile:33) — is never mentioned. The diagram (lines 7-12) shows `infrastructure/ config, logging, output` with no `.ckeletin/` entry, mirroring the stale AGENTS.md layout (the actual config.rs/logging.rs/output.rs live in .ckeletin/crate/src/; crates/infrastructure/src/lib.rs is a 7-line re-export facade).
- **Why it matters:** A new adopter landing on GitHub sees no path from 'this is a scaffold' to 'make my project from it', and the only architecture picture omits the directory that the whole update/propagation story revolves around. Distinct from the confirmed AGENTS.md finding: this is the public-facing copy of the same rot, plus a discoverability gap AGENTS.md doesn't have.
- **Recommendation:** Add a 'Start a new project' section (`just init myproject`) to the Quick Start, and add `.ckeletin/` to the diagram (or point to AGENTS.md once that diagram is fixed — then keep only one diagram per Principle 7).
- **Verification:** confirmed

## Severity: info

### clap usage errors in JSON mode bypass the envelope (human text, exit 2)

- **File:** `crates/cli/src/main.rs:57`
- **Ref:** CKSPEC-OUT-002
- **Evidence:** `ckeletin-rust --output json bogus` -> exit 2, stdout empty, stderr `error: unrecognized subcommand 'bogus'` + usage text. parse_args lets clap's `get_matches()`/`Error::exit` handle parse failures before output-mode handling exists. [dims: correctness,errors]
- **Why it matters:** Stdout stays clean (no stream-mixing; an agent gets empty stdout + exit 2, which is unambiguous), and arg-parse errors arguably precede 'the command' existing, so the OUT-002 envelope MUST doesn't squarely apply. Recorded as an observation: agents driving the CLI must handle exit 2 + non-envelope stderr as a distinct failure class from exit 1 + envelope.
- **Recommendation:** Either document the exit-2/no-envelope contract in AGENTS.md's agent-facing notes (and the catalog docs), or intercept clap errors when `--output json` / CKELETIN_JSON is detectable in raw args and wrap them in an error envelope.
- **Verification:** confirmed

### conform: spec-load failures print human text even in --json mode

- **File:** `.ckeletin/conform/src/main.rs:129`
- **Ref:** —
- **Evidence:** load_spec_requirements error branches use bare `eprintln!` + exit(1) (lines 129-133 and 143-146) regardless of json_mode, while the later failure modes (version mismatch line 327, ENF-005 line 345, ENF-008 line 375, ENF-010 line 414) all emit {"status":"error",...} JSON on stdout when json_mode is set. [dims: correctness]
- **Why it matters:** Inconsistent machine-readability within the same tool: an agent driving `conform --json` parses stdout JSON for four failure classes but must fall back to stderr text for missing/corrupt vendored spec. Dev-tool polish only.
- **Recommendation:** Route the two load_spec_requirements error branches through the same json_mode-aware error printer used by the other gates.
- **Verification:** confirmed

### Stray audit log artifact in the source tree at crates/cli/logs/

- **File:** `crates/cli/logs/app.log.2026-05-29`
- **Ref:** —
- **Evidence:** crates/cli/logs/app.log.2026-05-29 exists in the working tree (untracked/ignored — `git ls-files crates/cli/logs/` is empty and the tree is clean). The integration tests use tempfile::tempdir, and resolve_audit_path anchors relative paths under ~/.config/<app>, so this is likely a leftover from a pre-resolve_audit_path run writing the relative default `logs/app.log` from cwd=crates/cli. [dims: architecture]
- **Why it matters:** Harmless residue, but it is physical evidence the relative-path fallback once wrote inside the source tree; worth deleting so nobody mistakes it for current behavior.
- **Recommendation:** Delete the file; optionally assert in a test that running the binary from a crate directory does not create ./logs/.
- **Verification:** confirmed

### Vendored spec snapshot strips requirement metadata (level, title, domain)

- **File:** `conformance/requirements.json:3`
- **Ref:** CKSPEC-ENF-005
- **Evidence:** Upstream spec/requirements.json carries id, title, level (MUST/SHOULD), checkable, domain, since per requirement; the vendored conformance/requirements.json contains only {"id": ...} entries plus spec_version (the --refresh writer at main.rs:106-111 serializes id only). The ID set and spec_version match upstream 0.8.0 exactly (verified by jq diff: all 40 IDs identical). [dims: spec-conformance]
- **Why it matters:** Sufficient for the completeness gate (IDs are the anchor, per ENF-005), so no conformance defect. But the generator cannot distinguish a MUST from a SHOULD when reporting, and a spec bump's vendored diff shows bare IDs instead of reviewable titles/levels — slightly weakening the 'review the diff and reconcile deliberately' workflow the refresh comment promises.
- **Recommendation:** Have --refresh preserve title and level in the vendored snapshot. Cheap, improves spec-bump review diffs, and enables future MUST-vs-SHOULD-aware reporting.
- **Verification:** confirmed

### Justfile `binary_name` variable is dead — defined, dutifully rewritten by init.sh, consumed by nothing

- **File:** `Justfile:6`
- **Ref:** Principle 5 — Platforms, Not Features; Principle 7 — SSOT
- **Evidence:** Justfile:6 `binary_name := "ckeletin-rust"`; .ckeletin/scripts/init.sh:52 rewrites it on init; grep '{{binary_name}}' across Justfile and .ckeletin/Justfile → zero uses (exit 1). Meanwhile the real binary name truth lives in crates/cli/Cargo.toml:9 ([[bin]] name) and is hardcoded again in release.yml ('target/release/ckeletin-rust'). [dims: manifesto]
- **Why it matters:** A knob that init maintains but nothing reads is leftover scaffolding — and a third copy of the binary-name truth. Anyone changing it expects an effect and gets none.
- **Recommendation:** Either use it (e.g. a `run` recipe: `cargo run --bin {{binary_name}}`, or have release.yml-equivalent tooling read it) or delete the variable and the init.sh rewrite line.
- **Verification:** confirmed

### .gitignore comment claims audit logs default to './logs/' — contradicting the actual per-user-config-dir default documented everywhere else

- **File:** `.gitignore:7`
- **Ref:** Principle 7 — SSOT
- **Evidence:** .gitignore:7: '# Audit logs (CKSPEC-OUT-004 — written to ./logs/ by default)'. But AGENTS.md:12 says 'on by default, at ~/.config/<app>/logs/', CONFORMANCE.md OUT-004 says 'stable per-user path (~/.config/<app>/logs/ by default)', and .ckeletin/crate/src/config.rs defaults log_location to "config" (test: assert_eq!(config.log_location, "config")). A stray runtime artifact crates/cli/logs/app.log.2026-05-29 sits (ignored) in the tree, suggesting the comment described an older cwd-relative default. [dims: manifesto]
- **Why it matters:** Three documents describe the default log location; one is stale. Minor, but it is the kind of copied fact Principle 7 says to reference, not restate.
- **Recommendation:** Fix the .gitignore comment (the logs/ ignore pattern is still useful for tests/overrides) and delete the stray crates/cli/logs/ artifact.
- **Verification:** confirmed

### Spec-repo rationale citations use two different principle numberings — 01/03 yamls disagree with principles.md (cross-repo observation)

- **File:** `/Users/peiman/dev/ckeletin/spec/01-architecture.yaml:28`
- **Ref:** Principle 7 — SSOT
- **Evidence:** 01-architecture.yaml:28 'Separation of Concerns (Principle 5)' and :31 'Platforms, Not Features (Principle 4)'; :48 'Automated Enforcement (Principle 2)'; :65 'Framework Independence (Principle 6)' — a principle name that doesn't exist in principles.md (6 = Partnership). 03-testing.yaml uses the same old scheme ('Lean Iteration (Principle 3)'). 04-output.yaml uses the correct principles.md numbering ('Separation of Concerns (Principle 8)', 'Automated Enforcement (Principle 9)', 'SSOT (Principle 7)'). principles.md is the declared SSOT: 1 Truth-Seeking … 10 Feedback Cycle. [dims: manifesto]
- **Why it matters:** Out of ckeletin-rust's scope but it corrupts the traceability chain this repo depends on: CONFORMANCE.md cites principles by number, and an agent tracing a requirement's rationale through 01/03 gets the wrong (or a phantom) principle. Within ckeletin-rust itself all principle citations checked (CONFORMANCE.md Principles 1/10) are correct.
- **Recommendation:** In the spec repo: renumber 01-architecture.yaml and 03-testing.yaml rationales to the principles.md scheme, and add a numbering-consistency check (grep-able) to the spec's own gate.
- **Verification:** confirmed

### Minor SSOT copies: coverage '85' literal repeated in prose; lefthook fmt hook inlines the command its sibling routes through just; spec URL in two places

- **File:** `lefthook.yml:6`
- **Ref:** Principle 7 — SSOT
- **Evidence:** (a) lefthook.yml:6 fmt hook runs `cargo fmt --all -- --check` inline while the clippy hook (line 11) runs `just ckeletin-clippy` with the comment 'SSOT: run the framework's hardened clippy recipe... so pre-commit and `just check` enforce the same set' — fmt doesn't follow its own stated pattern (`just ckeletin-fmt-check` exists). (b) The operative 85% threshold lives once in Justfile:26 (good — CI calls `just coverage`), but the literal is copied in AGENTS.md:14/:134, ci.yml:132 job name, CONFORMANCE.md and the mapping evidence. (c) The spec URL https://raw.githubusercontent.com/peiman/ckeletin/main/spec/requirements.json is hardcoded in both .ckeletin/conform/src/main.rs:8 and .github/workflows/spec-drift.yml:37. [dims: manifesto]
- **Why it matters:** Each is currently harmless (the commands are identical, the threshold and URL haven't moved), but they are exactly the small duplications Principle 7 says drift first — and the fmt hook contradicts the SSOT rationale written two lines below it.
- **Recommendation:** Point the lefthook fmt hook at `just ckeletin-fmt-check`; phrase doc references as 'the threshold in `just coverage`' where practical; have spec-drift.yml ask the conform crate for the URL (or accept and document the pair).
- **Verification:** confirmed

### Guard tests silently pass (skip) when just/rsync are missing from PATH

- **File:** `.ckeletin/crate/tests/update_guard.rs:46`
- **Ref:** CKSPEC-ENF-001
- **Evidence:** `if !have("just") || !have("rsync") { eprintln!("SKIP update_guard: ..."); return; }` — the test reports ok. Same pattern in conform_guard.rs:40 and doctor.rs:33/73. A CI runner image change that drops rsync would turn the update-guard, conform-guard, and doctor tests into green no-ops with only an eprintln (invisible unless someone reads logs). [dims: tests]
- **Why it matters:** A green `just check` is the repo's single trust gateway (CKSPEC-ENF-001); tests that can quietly self-disable weaken what green means. (Mitigations exist: `just check` itself proves `just` is present, and init_smoke panics loudly if rsync is missing — so exposure is mainly conform_guard/update_guard/doctor under rsync loss.)
- **Recommendation:** Fail (not skip) when the tool is missing and an env var like CI=true is set: `assert!(have("rsync") || std::env::var("CI").is_err(), ...)`. Keeps the local-dev convenience while making CI self-disabling impossible.
- **Verification:** downgraded

### fuzz_ping invariants are near-tautological for the shipped type — acceptable as a worked example, nil as a current guard

- **File:** `crates/domain/tests/fuzz_ping.rs:26`
- **Ref:** CKSPEC-TEST-001
- **Evidence:** The properties are: Display never panics (PingResult's Display is `write!(f, "Pong! {}", message)` over a String — cannot panic for any input) and serde_json round-trips a String field (serde_json String serialization cannot fail). For the type as shipped, no input bolero generates can falsify either property. The header is honest about this: 'Replace PingResult with your own type/parser when you add real input handling.' The bounded pass does run (1.00s under cargo test, so real generation occurs). [dims: tests]
- **Why it matters:** Judged as exemplar code this is fine — it teaches the bolero-on-stable pattern, and the docstring disclaims it. Worth recording so the 'fuzzing' line in the feature list isn't mistaken for active input-space exploration of meaningful invariants: the value only materializes once a consumer points it at a real parser.
- **Recommendation:** No change required. Optionally make the example minimally falsifiable (e.g. fuzz a tiny parse function with a real edge case) so consumers see a fuzz test that could actually fail.
- **Verification:** confirmed

### Error-envelope subcommand identification is regression-tested only for ping; version/catalog name mappings unverified

- **File:** `crates/cli/src/main.rs:72`
- **Ref:** CKSPEC-OUT-003
- **Evidence:** subcommand_name maps Ping→"ping", Version→"version", Catalog→"catalog". The exhaustive match guarantees a new variant must declare *a* name (good — documented as the earned fix for the hardcoded-"init" bug), but nothing verifies the names are *correct*: json_mode_error_envelope_identifies_failing_subcommand (cli.rs:153) tests only ping. A copy-paste mapping Catalog→"version" would pass the whole suite. [dims: tests]
- **Why it matters:** The compile-time exhaustiveness defends against the original bug class (missing mapping), but the string values themselves are unguarded — minor because wrong strings require an unlikely edit, and the catalog command derives names structurally from clap elsewhere.
- **Recommendation:** Either add a unit test asserting subcommand_name agrees with the clap tree (iterate Cli::command().get_subcommands(), parse each name into Commands, assert round-trip), or extend the error-envelope integration test to one more command.
- **Verification:** confirmed

### Windows code paths (USERPROFILE/APPDATA log resolution) are entirely untested; suite and CI are Unix-only

- **File:** `.ckeletin/crate/src/logging.rs:71`
- **Ref:** CKSPEC-TEST-002
- **Evidence:** logging.rs implements `#[cfg(target_os = "windows")] { env_abs("APPDATA") }` and home() falls back to USERPROFILE, but CI runs ubuntu-latest only (ci.yml: every job `runs-on: ubuntu-latest`), and tests assume Unix: prepare_file_appender_fails_on_invalid_path relies on /dev/null/impossible (logging.rs:267), process.rs tests shell out to `echo`/`true`/`false`, cli.rs audit tests redirect XDG_CONFIG_HOME. [dims: tests]
- **Why it matters:** Not a defect — but the framework ships cfg'd Windows branches that have never executed under test, an honest-coverage caveat for any consumer targeting Windows (the cfg'd lines also silently drop out of the coverage denominator on Linux).
- **Recommendation:** Either document 'Unix-only verified' in AGENTS.md, or add a windows-latest CI job (even just `cargo test -p ckeletin`) if Windows is a supported target.
- **Verification:** confirmed

### build.rs degrades to 'unknown' silently at build time — correct per spec, but a cargo:warning would surface it sooner

- **File:** `crates/cli/build.rs:37`
- **Ref:** CKSPEC-OUT-006
- **Evidence:** `.unwrap_or_else(|| "unknown".to_string())` (build.rs:37, :41). The degradation IS visible at runtime — CKSPEC-OUT-006 explicitly requires 'unknown' over fabrication, version_line renders `0.1.0, commit unknown, built unknown`, and the single-atomic-`git describe` design (build.rs:21-29) eliminates the false-clean two-command trap. But nothing is emitted at build time, so a packager building outside a git checkout learns only when someone runs `--version`. [dims: errors]
- **Why it matters:** Spec-compliant honest degradation — this is a strength, not a defect. The only improvement is earlier visibility for the person producing the artifact.
- **Recommendation:** Optionally emit `println!("cargo:warning=git identity unresolved; baking commit=unknown")` when the describe call fails.
- **Verification:** confirmed

### Release ships a bare binary: no checksums, no provenance attestation, SBOM not attached; conform job over-granted contents:write

- **File:** `.github/workflows/release.yml:56`
- **Ref:** CKSPEC-ENF-009
- **Evidence:** release.yml:59-64: `gh release create "${GITHUB_REF_NAME}" --title ... --generate-notes target/release/ckeletin-rust` — the only artifact is the unadorned binary. No SHA256SUMS, no `actions/attest-build-provenance`, and the CycloneDX SBOM generated in CI (ci.yml:246-252) is only a workflow artifact with default ~90-day retention, never attached to the release it describes. Also `permissions: contents: write` at release.yml:19-20 is workflow-level, so the read-only `conform` gate job inherits write. [dims: security]
- **Why it matters:** Consumers downloading the release binary have no way to verify integrity or provenance, which undercuts the otherwise strong SBOM/grype story — the supply-chain evidence exists but is not bound to the shipped artifact. The header comment (release.yml:10-13) honestly frames this as 'minimal and hand-rolled on purpose', so this is a calibrated gap, not a broken guarantee. The build itself is clean: no cache reuse, `cargo build --release --locked`, and `${GITHUB_REF_NAME}` is env-expanded (not `${{ }}`-interpolated), so no injection.
- **Recommendation:** Attach `sha256sum` output and the sbom.cdx.json to the release, and add `actions/attest-build-provenance` (needs `id-token: write`, `attestations: write` on the publish job only). Move `contents: write` down to the publish job and give conform `contents: read`.
- **Verification:** downgraded

### init.sh leaves cosmetic scaffold prose in derived projects (self-referential about-string, untouched README/AGENTS)

- **File:** `.ckeletin/scripts/init.sh:46`
- **Ref:** —
- **Evidence:** root.rs doc comment is "A production-ready Rust CLI built with ckeletin-rust"; init.sh line 46 (`sedi "s/ckeletin-rust/$NAME/g" crates/cli/src/root.rs`) turns it into "A production-ready Rust CLI built with <name>" — this becomes the project's --help banner and catalog `description` (verified: catalog emits it). README.md/AGENTS.md keep upstream-specific prose (clone URL, project name) and init_smoke's stale-reference grep covers only crates/. [dims: framework-dx]
- **Why it matters:** Every derived project ships a slightly nonsensical self-description ('built with myproj') in its --help and machine catalog, and a README describing the scaffold rather than the project. Pure polish — nothing breaks.
- **Recommendation:** Have init.sh set the about-string to something neutral (e.g. "<name> — built on the ckeletin-rust scaffold" or just "<name>") and either templatize README.md or print a post-init reminder to rewrite README/AGENTS prose.
- **Verification:** confirmed

### Framework unit tests mutate process environment (std::env::set_var) with no serialization — racy under the `cargo test` fallback and the CI coverage job, and a hard blocker for edition-2024 migration

- **File:** `.ckeletin/crate/src/config.rs:176`
- **Ref:** Principle 9 — Automated Enforcement
- **Evidence:** Five tests call bare `std::env::set_var`/`remove_var` (config.rs:176-238, e.g. `std::env::set_var("CKROBUST_LOG_LEVEL", "trace")`) with no mutex/serial marker. Unique prefixes avoid functional collisions, but figment's Env provider iterates the whole environment while sibling tests mutate it. Under nextest this is safe (process-per-test), but Justfile:14 is `cargo nextest run --workspace 2>/dev/null || cargo test --workspace` (threaded fallback) and the CI coverage gate runs `cargo llvm-cov --workspace` (Justfile:26), which drives plain threaded `cargo test` — so the racy configuration runs on every CI pass. Concurrent setenv/getenv is the documented soundness hazard that made set_var `unsafe` in Rust edition 2024; the workspace is edition 2021 (Cargo.toml:7).
- **Why it matters:** This is vendored framework code (.ckeletin/crate) that propagates downstream: a rare segfault/flake in the coverage job would be near-undiagnosable, and any consumer bumping to edition 2024 gets compile errors in framework-owned tests. It is also the one concurrency hazard in a codebase nine reviewers otherwise found clean.
- **Recommendation:** Wrap env-mutating tests in a shared static Mutex (or use the `temp-env` crate's closures); that removes the race on all runners and confines the unsafe block when the edition is eventually bumped.
- **Verification:** downgraded

### No GitHub Actions concurrency groups — every push to an open PR runs the full 7-job pipeline to completion alongside the superseded run

- **File:** `.github/workflows/ci.yml:3`
- **Ref:** —
- **Evidence:** `grep -n concurrency .github/workflows/*.yml` returns no matches in any of ci.yml, release.yml, spec-drift.yml. ci.yml runs check, init-smoke (full from-scratch build), coverage, conform, secret-scan (full-history), and sbom on every push/PR with cargo-install steps in each job.
- **Why it matters:** Pure cost/latency: rapid pushes to a PR queue redundant full pipelines (each job cargo-installs its tools). No correctness impact.
- **Recommendation:** Add `concurrency: { group: ${{ github.workflow }}-${{ github.ref }}, cancel-in-progress: true }` to ci.yml (not to release.yml, where cancelling a half-published release would be worse).
- **Verification:** confirmed

### init.sh rewrites everything except the LICENSE files — every derived project ships 'Copyright 2026 Peiman Khorramshahi' and the (abridged) Apache text as its own licensing

- **File:** `.ckeletin/scripts/init.sh:1`
- **Ref:** —
- **Evidence:** `grep -in 'license\|copyright' .ckeletin/scripts/init.sh` returns nothing, while the script rewrites the project name, slug, CHANGELOG, and git history. LICENSE-MIT line 3 and LICENSE-APACHE's appendix block both read 'Copyright 2026 Peiman Khorramshahi', and crates/*/Cargo.toml inherit `license.workspace = "MIT OR Apache-2.0"` (Cargo.toml:11) — all of which survive `just init` verbatim into the new repo.
- **Why it matters:** A derived (possibly proprietary) project silently publishes manifests declaring it MIT OR Apache-2.0 under the framework author's copyright. Conventional for templates, but this template's explicit ambition is unattended downstream propagation, and init.sh's siblings rewrite far less load-bearing strings.
- **Recommendation:** Either have init.sh print a post-init checklist item ('review/replace LICENSE-* and the workspace license field') or interactively rewrite the copyright holder line the way it rewrites the project name.
- **Verification:** confirmed

---

## Refuted findings (reviewer claims that did not survive adversarial verification)

### ~~ENF-002 evidence states wrong enforcement-level counts (claims 3 honor-system; actual is 9)~~ (claimed medium)

The finding compares two different populations. The ENF-002 evidence ("6 compile-time, 5 pre-commit/CI, 3 honor-system. See enforcement audit table.") counts architectural DECISIONS in the ENF-004 audit table — git log -L shows it was written as a verbatim quote of the audit table's own summary line in the same commit (07a75ac: "6 decisions enforced at compile-time... 5 decisions at pre-commit/CI... 3 decisions at honor system"). The reviewer instead counted the enforcement_level metadata field across the 40 REQUIREMENT mappings (9 honor-system), which per the spec schema (_schema.yaml: "Where on the enforcement ladder this requirement is verified") measures how each spec requirement's conformance is checked — a different axis. The spec rationale the finding cites ("many honor-system entries signal enforcement debt") belongs to ENF-004, which mandates a table of "every architectural decision" — the decision population. Checked against the current audit table in CONFORMANCE.md: 6 compile-time decision rows and exactly 3 honor-system decisions (TDD / atomic commits / changelog curation) — so the headline claim "claims 3 honor-system; actual is 9" is false in the evidence's own (correct) frame. No debt is hidden: conformance-report.json carries structured enforcement_level for all 40 requirements, so the 9-honor-system requirement histogram is machine-derivable by any aggregator regardless of the prose. Residual kernel: "5 pre-commit/CI" is mildly stale (the table grew), but the evidence defers to the table as SSOT, and the finding's recommended fix (replace with 7/15/3/6/9) would corrupt the evidence by substituting the wrong population. The core claim misreads the code.

### ~~Release gate never consults the live spec — up to a week of release-while-stale latency~~ (claimed low)

The finding's factual chain is accurate — release.yml's publish gates only on the hermetic conform job (needs: conform; .ckeletin/conform/src/main.rs touches the network only with --refresh), the weekly spec-drift.yml cron (0 7 * * 1) is the sole live-spec comparison and only opens an issue, so a release-while-stale window of up to a week (plus human reconcile time) genuinely exists. But verdict rules classify intentional-and-documented divergences as refuted, and this is squarely that: (1) the trade-off is documented in four places — the spec-drift.yml header comment explicitly framing the cron as the counterweight to deliberately-hermetic conform, the conformance-mapping.toml ENF-009 evidence, CONFORMANCE.md's ENF-009 paragraph plus its audit-table row that candidly scopes the limit ("Detects drift; reconciling it is a human action"), and commit 2596148's message describing the deliberate divergence from ckeletin-go's live-fetch; (2) decisively, the spec repo's authoritative conformance record (/Users/peiman/dev/ckeletin/conformance/ckeletin-rust.yaml lines 250-261) records ENF-009 as met describing exactly this mechanism — the spec author (same person, n=1 ecosystem by design) has accepted scheduled drift detection as satisfying the "verify against latest" clause; (3) the implicit comparator, ckeletin-go's live-fetching conform, also lacks a hard release-time guarantee (conform.sh silently falls back to cache/hardcoded list on fetch failure) and was likewise accepted as met, showing the spec author's interpretation is consistent; (4) the finding's own "why" concedes the refutation ("honestly documented... met-in-letter... spec's notes allow mechanism choice"). The residual letter-tension with "drift from the latest spec blocks the release" is resolved by the spec author's recorded acceptance in the spec repo's own conformance record. The recommendation (a live version probe in release.yml) is a cheap, sensible future enhancement relevant to the autonomous-maintenance endgame, but it is a feature request against an accepted, documented design — not a defect to carry as a finding.

### ~~Published report's `passed: true` reflects declared statuses, not executed check results~~ (claimed info)

The finding's code reading is correct (PublishedSummary.passed = `partial == 0 && deferred == 0` at /Users/peiman/dev/ckeletin-rust/.ckeletin/conform/src/main.rs:280; project_report runs at :392 before checks at :427+), but it fails as a finding on four counts. (1) Spec-mandated by design: the spec's ENF-010 definition (/Users/peiman/dev/ckeletin/spec/02-enforcement.yaml:270+) explicitly requires the report to be "a per-requirement projection of its own conformance mapping (status, evidence, and the checks or tests that anchor each claim)" — declared status is the required content, not runtime check results. The "cross-impl schema note in the spec repo" the reviewer recommends already exists as the requirement's own description and notes, which any aggregator author would read. (2) The claim that the semantics "currently lives only in a code comment" is wrong: CONFORMANCE.md:73 states the report "is a deterministic projection of conformance-mapping.toml ... regenerates it in memory and fails if the committed file drifted" — i.e., projection-of-declared-mapping semantics are documented in CONFORMANCE.md's ENF-010 section, plus the thorough code comment at main.rs:237-245. (3) The hypothesized consumer doesn't exist: the only real consumer, /Users/peiman/dev/ckeletin/scripts/aggregate_conformance.py, never reads `summary` or `passed` (grep finds zero field usage — only docstring hits about CLI stderr summaries); it consumes per-requirement status + evidence and stamps its own report_date. (4) Already mitigated in practice, as the reviewer concedes: the CI `conform` job (.github/workflows/ci.yml:163) sync-checks the committed report AND runs every mapped check on the same gate, and release.yml gates publish on it, so a committed `passed: true` on green main implies the checks did run green. The field name deliberately mirrors ckeletin-go's schema for cross-impl aggregation. The finding itself states "no action strictly required" and "honestly documented in code" — it is an observation about intentional, documented, spec-conformant behavior with no extant consumer at risk, which under the verdict rules (intentional-and-documented, already mitigated) is refuted.

### ~~init.sh sed renames are silent no-ops when patterns don't match — drifted upstream text yields a half-renamed project~~ (claimed medium)

Already mitigated — the reviewer's exact recommendation is implemented as a CI-gated smoke test they missed. The mechanical premise is correct (sed no-ops exit 0; init.sh's own verify at line 88 is only `cargo check`), but the claim that 'a drifted pattern sails through' is false: /Users/peiman/dev/ckeletin-rust/.ckeletin/crate/tests/init_smoke.rs runs init.sh end-to-end in a temp copy and asserts (lines 98–114) that `grep -r ckeletin-rust --include=*.rs --include=*.toml crates/` returns nothing after init — the literal check the finding recommends — plus explicit assertions that the binary name was patched in crates/cli/Cargo.toml (line 118), the env prefix in main.rs (line 123), and that the initialized project's full `cargo test --workspace` passes (lines 140–150). This test is wired into CI as the `init-smoke` job (.github/workflows/ci.yml lines 99–129, via `just init-smoke`), running on every push and PR to main upstream. Since pattern drift can only originate upstream, drifted text cannot land on main — let alone a release consumers init from — without this job failing first. Both concrete drift examples in the finding's evidence (reformatted Cargo.toml name line, changed ping message) live in crates/ and are caught by the stale-reference grep and/or the workspace test run. The only sed targets outside the smoke grep's scope are the root Cargo.toml `repository` URL (step 2) and the Justfile `binary_name` variable (step 3); both are cosmetic — `binary_name` is declared at Justfile line 6 and referenced by zero recipes, and the repository URL is metadata a consumer updates anyway — neither produces the claimed silent half-rename with test mismatches. The remaining theoretical gap (a user running init from a locally modified scaffold, bypassing upstream CI) is outside the finding's stated 'drifted upstream text' scenario and at most an info-level hardening suggestion, not a medium-severity enforcement gap. Principle 9 enforcement exists; it lives in CI at the drift source rather than inside init.sh.

### ~~The vendored framework compiles two incompatible TOML stacks — figment pins toml 0.8.2 for ckeletin while ckeletin-conform pulls toml 1.1.2 — and deny.toml only warns on duplicates~~ (claimed info)

The finding's raw facts are accurate — `cargo tree -d` confirms toml 0.8.2 (figment <- ckeletin) coexisting with toml 1.1.2+spec-1.1.0 (ckeletin-conform), plus duplicate serde_spanned/toml_datetime/winnow, and deny.toml:21 sets multiple-versions = "warn". But the finding is refuted as intentional-and-documented. Commit 95b9d36 ("chore(deps): bump toml 0.8->1 in conform + routine cargo patch bumps") shows the split was a deliberate decision resolving Dependabot #9-#13, with the rationale written explicitly in the commit body: "figment keeps toml 0.8 on the config path — two majors coexist, normal." The finding's core "why" — that this is drift the repo "should at least be tracking deliberately" — is contradicted: it was raised by Dependabot, consciously decided, documented in commit history, and is re-surfaced on every cargo-deny run (cargo deny check runs in `just check` via .ckeletin/justfile:101-104 and in CI via .github/workflows/ci.yml). The finding's own recommendation (keep multiple-versions=warn, treat duplication as known/accepted, revisit when figment releases toml>=1) exactly describes the existing state, so there is no actionable gap. The finding itself concedes "Not a defect."

---

## Residual coverage gaps (honestly not closed)

- GitHub remote settings were not audited: branch protection / required status checks (whether `just check`, conform, and coverage are actually required to merge) are invisible from the working tree; gh release list and gh run list returned empty, which I used as evidence that release.yml has never fired, but I did not page through historical CI runs.
- The set_var concurrency hazard is asserted from code semantics (threaded cargo test under cargo-llvm-cov), not reproduced empirically — the race is probabilistic and I did not attempt a stress reproduction.
- Windows code paths (USERPROFILE/APPDATA resolution) remain unexercised on this macOS machine — inherited gap from the tests reviewer, could not be closed locally.
- The LICENSE-APACHE analysis is a textual diff against the canonical Apache-2.0 text (via serde's vendored copy); the legal significance of each dropped clause is my lay reading, not counsel.
- I did not execute `just init` / init.sh or `just ckeletin-fuzz` end-to-end locally (init is destructive by design; CI init-smoke covers the former per prior reviewers).
- docs/spec-feedback-2026-05.md, docs/plans/*, and the spec repo's six YAML files were only spot-checked for license/changelog-related requirements, not re-audited line-by-line — prior reviewers' coverage was assumed for their substance.
- Cargo.lock was checked for duplicate versions and source hygiene via cargo tree and the green cargo-deny ground truth, but not audited entry-by-entry for typosquats or unexpected transitive additions.
