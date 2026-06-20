# Example: an ISO 27001 audit scope as live glue

A worked, runnable example of using muster for an **audit** — here, an ISO 27001
scope where every control is wired at the **real evidence** it claims, so the
readiness verdict is an honest pre-audit truth-meter and nothing can be presented
greener than reality.

If [`ckeletin-feedback`](../ckeletin-feedback/) shows muster *governing a
process*, this shows muster *answering "are we actually audit-ready?"* — the
question a consultant charges for, answered from your own artifacts.

## What it sets up

An `isms` process (the spine) plus five controls mapped to Annex A clauses, each
deriving Pass/Fail by **resolving a real source on every read** — never from a
value copied into muster:

| Control | Clause | Reads, live | Green only when |
|---|---|---|---|
| `a8-29` | A.8.29 Security testing | `evidence/coverage.json` → `summary.line_percent`, `--expect ">=80"` | coverage ≥ 80% |
| `a8-13` | A.8.13 Information backup | `evidence/backup-drill.json` → `last_restore.status` | the last restore drill is `met` |
| `a5-18` | A.5.18 Access rights | `evidence/access-review.json` → `review.days_since`, `--expect "<=90"` | reviewed within 90 days |
| `a8-12` | A.8.12 Data leakage | `checks/secrets_scan.sh` (a live command, re-run each read) | the secrets scan exits 0 |
| `a5-1`  | A.5.1 Policies | a `note` only | **never** — it is *asserted*, surfaced as unverified until evidenced |

The three ref kinds are all here: a **metric + a bar** (`--expect`), a **report
anchor** (`--ref-report`), and a **live command** (`--ref-cmd`). The fifth, `a5-1`,
is the audit-grade honesty: a hand-waved "we have a policy, trust me" never reads
green.

## Run it

With muster installed (see the repo README → Install) and on your `PATH`:

```sh
cd examples/audit
./setup-audit.sh         # defines the ISMS + the 5 clause-mapped controls in ./.muster
muster readiness         # the honest pre-audit truth-meter
```

You'll see the real verdict — **not** a rubber-stamp:

```
readiness: GAPS: 1
  control coverage: 4/5 applicable implemented-with-evidence (80%)
    gap: a5-1 — status is not_started, not implemented-with-evidence
  controls (derived): a5-18, a8-12, a8-13, a8-29
  controls (asserted, unverified): a5-1
  proven: (none)
  asserted: isms
  enforcement:
    isms — none [no_enforcement]
  drift profile (ref-kind honesty):
    a5-18 — live_resolved
    a8-12 — live_resolved
    a8-13 — live_resolved
    a8-29 — live_resolved
```

Four controls prove themselves from evidence; the policy control is honestly held
back as *asserted*. Closing it shows the **evidence is not just a note** rule:

```sh
muster control attach-evidence a5-1 note "policy.md approved"
muster control set-status a5-1 implemented
muster readiness     # STILL a gap — a note is honor-level (control_honor_evidence)

muster control attach-evidence a5-1 file evidence/policy-signed.pdf
muster readiness     # now READY — a verifying artifact (file/url) is what counts
```

## The teeth (why this beats a checklist)

**1. Live re-resolution — no stale green at audit time.** `muster control show`
reads the source *now*, so a control can never out-live reality:

```sh
muster control show a8-29        # outcome: pass (coverage 91.4 ≥ 80)
# drop coverage below the bar in the evidence file:
#   "line_percent": 70
muster control show a8-29        # outcome: fail (70 < 80) — instantly, no muster edit
```

**2. Evidence can't go quietly stale.** Set an opt-in source-freshness bound and a
control whose evidence artifact is too old is flagged `ref_source_stale` — a
confident `met` from a report nobody regenerated since last year doesn't pass:

```sh
MUSTER_SOURCE_FRESHNESS_SECS=86400 muster readiness   # a year-old coverage.json → a8-29 ref_source_stale
```

**3. muster is an optional view.** The same bars enforce in CI with no muster at
all — `./ci-check.sh` reads the very same evidence files and fails the build if
any control's bar is unmet. `muster readiness` is that truth rendered as a
living, clause-mapped process map for the auditor.

## How a real audit maps onto this

- **Controls → clauses:** `--clause-ref "ISO 27001 A.8.29"` (or `muster control
  import <standard>.toml` to load a whole Annex A at once as references).
- **Glue → live evidence:** point each control at the artifact your tooling
  already produces (`--ref-file`/`--expect`, `--ref-report`, `--ref-cmd`).
- **Readiness = the pre-audit truth-meter:** the proven-vs-asserted split, the
  coverage %, and gaps-with-reasons are your honest gap analysis — and it stays
  red until the gaps are really closed.
