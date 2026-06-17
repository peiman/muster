# Spec feedback — ckeletin-rust → ckeletin (2026-06)

> **Status: submitted upstream as GitHub issues + draft PRs on peiman/ckeletin.**
> Produced by Wave 4 of the 2026-06-09 adversarial code review of ckeletin-rust,
> covering PRs #27, #29–#32 on peiman/ckeletin-rust. Each section is a concrete
> proposal grounded in cross-implementation evidence (Principle 10 — Feedback Cycle).

## Background

A full adversarially-verified code review of ckeletin-rust (9 quality dimensions,
June 2026) produced findings that required spec changes to prevent recurrence across
both implementations. This document records those findings, the evidence, and
proposed spec amendments. Where proposals are normative (MUST/SHOULD/MAY language
changes), draft PRs are opened on peiman/ckeletin.

Rust implementation: github.com/peiman/ckeletin-rust
Reviewed PRs: #27 (supply chain), #29 (license/hygiene), #30 (update mechanism),
#31 (audit-stream + output-mode), #32 (conformance trust chain).

---

## a. Anchor validity (ENF-008)

### Observation

CKSPEC-ENF-008 requires that every `met` conformance claim be anchored to
verifiable evidence — either an automated check or an analysis-with-evidence that
references the specific artifact(s). The generator is required to fail on an
unanchored claim.

However, ENF-008 does not require that a file-path anchor actually _exist_ or that
a `file::symbol` anchor contain the named symbol. A dangling path passes the
generator's anchor check silently.

The exact incident: after PR #31 renamed a shadow-log test, the conformance mapping
continued to cite `audit_log_written_by_default` (the old name). The mapping was
`met` with an anchor — a non-empty `violation_evidence` string that contained a
now-defunct test path. The generator accepted it. The anchor was meaningless.

PR #32 added a **dangling-anchor gate** to the `just conform` generator
(`.ckeletin/conform/src/main.rs`): every evidence string that looks like a file path
(contains `/`, ends in `.rs`/`.toml`/etc.) must exist on disk; a `file.rs::symbol`
anchor must have the symbol present in the file. This gate provably fails on the
renamed-test incident and caught three other stale anchors during remediation.

### Evidence

- Stale anchor incident: `.ckeletin/CHANGELOG.md` entry for v0.2.20, "OUT-004 stale
  anchor fixed" (PR #32).
- Gate implementation: `.ckeletin/conform/src/main.rs` (PR #32, Wave 2 / Finding #1).
- ENF-008 current text: `spec/02-enforcement.yaml`, lines 195–234 — requires anchor
  presence, silent on anchor validity.

### Proposal

Strengthen ENF-008 (or add CKSPEC-ENF-011) to require that conformance tooling
validate evidence anchors, not merely their presence:

1. File-path anchors MUST be verified to exist on disk at generator run time.
2. `file.rs::symbol` anchors MUST be verified to contain the named symbol.
3. A met claim whose anchor fails validation MUST cause the generator to fail, as if
   the anchor were absent.

Rationale: an anchor that names a non-existent file is no better than no anchor —
it provides false confidence (Principle 1 — Truth-Seeking). The generator checking
anchor presence but not validity is checking the letter of ENF-008, not its spirit.

---

## b. Enforcement-level vocabulary (_schema.yaml)

### Observation

The `_schema.yaml` `enforcement_level` enum lists valid values (`compile-time`,
`linter`, `sast`, `script`, `ci`, `honor-system`) with a brief description of the
ladder. It does NOT define what each level means precisely, nor does it require that
a claimed level be backed by evidence appropriate to that level.

Two incidents from PR #32:

1. **OUT-005 `compile-time` over-claim**: the conformance mapping claimed
   `enforcement_level: compile-time` for OUT-005 (output isolation) when the actual
   enforcement was a clippy lint (`print_stdout = "deny"`). The compiler does not
   enforce this — a `println!` in domain code compiles until clippy runs. PR #32
   corrected the level to `linter` and added the clippy deny to make the claim true.
   Evidence: `.ckeletin/CHANGELOG.md` v0.2.20, "OUT-005 enforcement level corrected"
   and "ARCH-006 enforcement level corrected".

2. **ARCH-006 `compile-time` over-claim**: entry-point minimality was claimed as
   `compile-time`; it is a design/structural property, not compiler-verified.
   Corrected to `design`.

Both corrections were made in PR #32 but neither was _caught_ by the conformance
tooling. A level claim is accepted without any proof that the claimed mechanism
works.

### Evidence

- Corrections: `.ckeletin/CHANGELOG.md` v0.2.20 "Fixed" section (PR #32).
- Schema vocabulary: `spec/_schema.yaml` lines 100–108 — enum values listed,
  no definitions.
- ENF-006 already requires `violation_test` or `violation_evidence` for claims
  above `honor-system` — but does not require the evidence to _match_ the level kind.

### Proposal

In `_schema.yaml`, add a `level_definitions` block under `conformance_entry` that
defines what each `enforcement_level` value means and what evidence kind it requires:

- `compile-time`: the language toolchain rejects violating code at compile time. A
  valid `violation_test` MUST be a compile-fail test (e.g., trybuild). A
  `violation_evidence` claiming compile-time enforcement MUST reference the specific
  Cargo.toml dependency omission that makes the import structurally impossible.
- `linter`: a configured linter (clippy, golangci-lint, etc.) catches violations as
  part of the standard check. Evidence MUST reference the specific lint rule and its
  configuration.
- `sast`: a static analysis tool beyond the standard linter. Evidence MUST reference
  the tool and rule.
- `script`: a script or conformance generator enforces the rule. Evidence MUST
  reference the script/recipe and the check it performs.
- `ci`: enforcement only fires in CI (not locally). Evidence MUST reference the CI
  job.
- `honor-system`: no automated enforcement. Evidence is analysis-with-justification.
  No `violation_test` is required or expected.

Additionally, add a normative requirement (new CKSPEC-ENF requirement or amendment
to ENF-006) that the `violation_test` or `violation_evidence` must be _consistent_
with the claimed `enforcement_level`: a `compile-time` claim backed by a script
check is a mis-claim, not a met requirement.

---

## c. OUT-002 flag-overrides-config precedence (normative gap)

### Observation

CKSPEC-OUT-002 requires machine-readable output mode activated "via flag or
configuration." The v0.8.0 notes add the `--output text|json` SHOULD. Neither the
requirement text nor the notes specify that an _explicit_ flag MUST override config
or environment variables in BOTH directions.

The rust incident (fixed in PR #31, HIGH severity): when `json = true` was set in
the config file, `--output text` was silently ignored — the config value won. An
explicit user flag was overridden by a stored preference. The fix introduced a
single `resolve_output_mode()` function (SSOT) with explicit precedence: CLI flag >
config > default, in both directions.

The Go implementation had this correct from the start. The spec gap meant neither
implementation had a normative test to catch the regression.

### Evidence

- Rust fix: PR #31 "Explicit `--output text` overrides `json = true` from
  config/env" (HIGH severity finding).
- PR #31 body: "mode resolution is a single `resolve_output_mode()` (SSOT —
  Principle 7): explicit flag > config > default, in both directions, for successes
  and errors."
- OUT-002 current text: `spec/04-output.yaml` lines 33–61 — no precedence language.

### Proposal

Add a normative sentence to CKSPEC-OUT-002:

> When both a flag and configuration (file or environment variable) specify the
> output mode, the explicit flag MUST take precedence in both directions: `--output
> text` MUST override a configured JSON mode, and `--output json` MUST override a
> configured text mode.

Additionally, add a required test to the conformance conformance checklist for
OUT-002: "flag overrides config in both directions" — verifiable by (1) setting
`json = true` in config, running with `--output text`, and asserting human output,
and (2) running with `--output json` and no config and asserting JSON output.

---

## d. OUT-004 audit-stream failure semantics (spec silent)

### Observation

CKSPEC-OUT-004 requires that every user-facing output operation simultaneously log
to the audit stream. It is silent on what happens when the audit stream cannot
initialize or when write operations fail.

Three incidents from PR #31 (all HIGH or directly related):

1. **Reachable panic on permission failure**: `tracing_appender::rolling::daily`
   panicked (exit 101, raw backtrace) when the audit log file could not be created
   — e.g., a root-owned log file after one `sudo` run. The fix: use the builder
   API's `Result`-returning variant; failure flows to the normal error envelope
   + exit 1.

2. **Error envelopes not reaching the audit log**: the `LogGuard` (the
   `WorkerGuard` returned by the non-blocking writer) was dropped before `run()`
   rendered errors, so `output.error` events went to a dead worker. The audit trail
   recorded command failures as if they succeeded. The fix: extend the guard's
   lifetime through the error-rendering path.

3. **Silent event drop on write failure**: shadow-log events were emitted _before_
   the user-facing write, so a failed write (e.g., broken pipe) produced a
   misleading `output.success` audit record for a run that never delivered output.
   Fix: emit the tracing event only after the write succeeds.

Additionally, PR #31 added audit directory 0700 / file 0600 permissions (Unix).
Audit logs contain every byte rendered to users by design; default 0644/0755
permissions exposed per-user audit contents to all local users.

### Evidence

- All three fixes: PR #31 body and `.ckeletin/CHANGELOG.md` v0.2.19.
- PR #31 body: "The `LogGuard` was dropped before `run()` rendered errors, so
  `output.error` events went to a dead worker — the audit trail recorded failures
  as successes."

### Proposal

Add the following normative text to CKSPEC-OUT-004:

1. **No panics on init failure**: audit stream initialization failure MUST be
   surfaced as a typed error (mapped to the normal error-envelope path and exit 1).
   A panic on log-file creation failure is a violation.

2. **Guard lifetime**: the audit stream writer's lifetime MUST cover the entire
   command execution, including error-output paths. An implementation MUST NOT drop
   the writer guard before rendering error output.

3. **Shadow log after write**: audit log events MUST be emitted only after the
   user-facing write succeeds. A failed write MUST NOT produce an audit record
   claiming success.

4. **File permissions** (SHOULD): on Unix systems, the audit log directory SHOULD
   be created with mode 0700 and log files with mode 0600, to prevent other local
   users from reading per-user audit contents.

---

## e. OUT-003 envelope: null vs omitted, flat vs structured error

### Observation

CKSPEC-OUT-003 specifies the envelope as: "status (success or error), command
identifier, data payload (**null on error**), and error details (**null on
success**)." The requirement uses the word "null" for the absent fields.

The two implementations diverge on interpretation:

**ckeletin-go** (`.ckeletin/pkg/output/json.go` lines 23–28):

```go
type JSONEnvelope struct {
    Status  string      `json:"status"`
    Command string      `json:"command"`
    Data    interface{} `json:"data"`    // nil on error — emits explicit null
    Error   *JSONError  `json:"error"`   // nil on success — emits explicit null
}
```

No `omitempty` tags. Emits `"data": null` on error and `"error": null` on success
as explicit JSON null values.

**ckeletin-rust** (`.ckeletin/crate/src/output.rs` lines 17–24):

```rust
pub struct Envelope {
    pub status: Status,
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
```

`skip_serializing_if = "Option::is_none"` omits absent fields entirely. A success
envelope has no `error` key; an error envelope has no `data` key.

The spec says "null on error" / "null on success." In JSON, `null` is an explicit
value distinct from field absence. The Go implementation matches the literal spec
text. The Rust implementation omits the fields.

**Second divergence — error shape**: the Go implementation uses a structured error
object `{"message": "...", "code": "..."}` (`JSONError` struct, optional `code`
field). The Rust implementation uses a flat string `error: String`. The spec text
says only "error details" — it does not specify a string vs. object shape.

### Recommendation

**On null vs. omitted**: the spec text ("null on error", "null on success") aligns
with Go's explicit-null approach. If the spec intends explicit null, it should say
so normatively. Recommendation: amend CKSPEC-OUT-003 to state explicitly:

> The `data` and `error` fields MUST be present in every envelope. When a field
> has no value, it MUST be serialized as JSON `null` rather than omitted. Consumers
> MUST be able to rely on a fixed set of envelope keys without checking for key
> presence.

This requires a Rust code change (remove `skip_serializing_if`, update tests) and
makes Go the reference for the envelope shape. The rationale: a fixed key set is
more predictable for consumers (Principle 7 — SSOT); key-presence checks are an
extra parsing step agents should not need.

**If the spec intends ambiguity**: leave the text as-is and add a note clarifying
that both explicit-null and omit-on-absent are conformant SHOULD implementations.
This preserves both current implementations but leaves agents unable to rely on a
fixed key set.

**On structured vs. flat error**: recommendation is to add a SHOULD for structured
error `{message, code?}` matching the Go shape, to be adopted by Rust in a future
framework bump. The richer shape enables error classification without a separate
metadata channel, and Go already demonstrates the pattern. The flat string is
technically conformant today (CKSPEC-OUT-003 is a SHOULD requirement); this would
be an upgrade, not a correction.

---

## f. ARCH-004 calibration: intra-crate module isolation

### Observation

CKSPEC-ARCH-004 requires that "Business Logic packages MUST NOT import each other."
The enforcement_level in conformance reports for Rust is claimed as `compile-time`
for the cross-crate boundary — Rust's Cargo dependency graph enforces this (domain
cannot import infrastructure/cli). However, the requirement says "packages" which
in the Rust single-domain-crate architecture maps to _modules within a crate_.

Intra-crate module isolation is NOT compile-time enforceable in Rust: `mod` items
in the same crate can freely reference each other via `pub` visibility within the
crate, and there is no compiler mechanism that prevents one business-logic module
from importing another without a separate crate boundary.

PR #32 corrected the ARCH-004 mapping entry with a note:
> "ARCH-004 at module level ('business logic packages MUST NOT import each other')
> is enforced at design level within the single domain crate — Rust cannot cheaply
> compile-time-enforce intra-crate module boundaries. The multi-crate split
> (domain/infrastructure/cli) is the documented upgrade path when domain grows."

The `compile-time` enforcement claim in the conformance mapping applies only to the
cross-crate boundary (domain cannot import infrastructure/cli). Within the domain
crate, isolation is a design-level property.

### Evidence

- Conformance mapping note: `conformance-mapping.toml`, CKSPEC-ARCH-004 notes field
  (PR #32 correction).
- ARCH-004 current text: `spec/01-architecture.yaml` lines 75–99.

### Proposal

Add a note to CKSPEC-ARCH-004 acknowledging the language-specific enforcement gap:

> In languages with a package system that maps directly to compile units (Go
> packages, Python packages), package-level isolation is enforceable by import
> analysis tools. In languages where a single compile unit may contain multiple
> logical "packages" (Rust modules within a crate), the strictest compile-time
> enforcement covers the crate boundary; intra-crate module isolation is enforced
> at the design level. The documented upgrade path for growing business-logic
> surface is to split into additional crates, enabling compile-time enforcement
> of the boundary.

The conformance schema's `enforcement_level` field should accept `design` as a
valid level for this case (it already does, per `_schema.yaml`), and conformance
reports should distinguish which boundary is compile-time enforced vs. design-level.

---

## g. Principle numbering inconsistencies between spec YAMLs and principles.md

### Observation

`principles.md` (the authoritative source) numbers the principles in this order:

| # | Name |
|---|------|
| 1 | Truth-Seeking |
| 2 | Curiosity Over Certainty |
| 3 | Good Will |
| 4 | Lean Iteration |
| 5 | Platforms, Not Features |
| 6 | Partnership |
| 7 | Single Source of Truth |
| 8 | Separation of Concerns |
| 9 | Automated Enforcement |
| 10 | Feedback Cycle |

The spec YAML rationale fields use principle numbers that were accurate under an
older principles structure (before the restructuring noted in the CHANGELOG:
"Principles restructured — derives from Manifesto"). The old structure had fewer
principles with different numbering. The newer files (04-output.yaml, later
additions to 02-enforcement.yaml) use the current numbering; the older files still
use the old numbering.

**Confirmed mismatches in `spec/01-architecture.yaml`**:

- `CKSPEC-ARCH-001` rationale: cites "Principle 5 (Separation of Concerns)" and
  "Principle 4 (Platforms, Not Features)" — these ARE correct under current
  numbering. But also cites **no name** for some principles, just numbers.
- `CKSPEC-ARCH-003` rationale: cites "**Principle 6** (Framework Independence)" —
  but `principles.md` has no "Framework Independence" principle (it was folded into
  Separation of Concerns). Current Principle 6 is "Partnership."
- `CKSPEC-ARCH-002` rationale: cites "**Principle 2** (Automated Enforcement)" —
  but current Principle 2 is "Curiosity Over Certainty." Automated Enforcement is
  Principle 9.

**Confirmed mismatches in `spec/02-enforcement.yaml`** (earlier requirements):

- `CKSPEC-ENF-001` rationale: "**Automated Enforcement (Principle 2)**" — should be
  Principle 9.
- `CKSPEC-ENF-002` rationale: "**Automated Enforcement (Principle 2)**" (line 47)
  and "**Lean Iteration (Principle 3)**" (line 53) — Lean Iteration is now
  Principle 4.
- `CKSPEC-ENF-003`, `ENF-004` rationale: "**Automated Enforcement (Principle 2)**".

The later requirements in the same file (`ENF-007`, `ENF-008`, `ENF-009`, `ENF-010`)
use **current** numbering correctly (Principle 9 for Automated Enforcement,
Principle 10 for Feedback Cycle). So the split is visible within the same file: old
requirements have old numbers, new requirements have new numbers.

**`spec/03-testing.yaml`**: all principle references use correct names (Truth-Seeking
Principle 1, Lean Iteration Principle 3 in TEST-001, Automated Enforcement
Principle 2 in TEST-002). Wait — Lean Iteration as Principle 3 in TEST-001 is the
old numbering (current: Principle 4). Automated Enforcement as Principle 2 in
TEST-002 is the old numbering (current: Principle 9). These are stale.

### Evidence

- `principles.md` current numbering: lines 20–100.
- `spec/01-architecture.yaml`: ARCH-003 rationale line 65, "Principle 6 (Framework
  Independence)"; ARCH-002 rationale line 48, "Principle 2".
- `spec/02-enforcement.yaml`: ENF-001 line 23 "Principle 2", ENF-002 lines 47/53
  "Principle 2/3", ENF-003 line 75 "Principle 2", ENF-004 line 97 "Principle 2".
- `spec/03-testing.yaml`: TEST-001 line 29 "Principle 3" (Lean Iteration), TEST-002
  line 46 "Principle 2" (Automated Enforcement).

### Proposal

This is a pure bugfix — the principle names are correct in all files (the name in
parentheses is right); only the numbers are stale. Fix: update all stale principle
numbers in 01-architecture.yaml, 02-enforcement.yaml, and 03-testing.yaml to match
the current `principles.md` numbering. Specifically:

- "Automated Enforcement (Principle 2)" → "Automated Enforcement (Principle 9)"
- "Lean Iteration (Principle 3)" → "Lean Iteration (Principle 4)"
- "Framework Independence (Principle 6)" → remove (folded into Separation of
  Concerns, Principle 8)

No semantic content changes — only number corrections. A draft PR for this fix is
the lowest-risk spec change in this batch.
