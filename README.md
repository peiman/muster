# muster

> *پندارِ نیک، گفتارِ نیک، کردارِ نیک* — good thoughts, good words, good deeds.
> *Skynda långsamt.*

**muster** is a minimalistic, AI-first command-line **ledningssystem** (management
system) for startups and midsize companies with **no compliance department**. Run
your management system as a living **process map**, become **ISO-certification-ready**
without consultants, and handle **incidents / command & control** — from one small,
honest tool, usable by AI agents and humans alike.

The name carries the thesis: *muster* the team and resources (incident C2) **and**
*pass muster* (meet the standard). Two jobs, one spine.

## The core idea — a process is a hypothesis

A process in muster is not true because someone wrote it down. It is a *hypothesis*
about how work should be done — tested by evidence, refuted by incidents and
nonconformities, strengthened by automated conformance checks. When reality
outgrows a process, the process is **revised**, and that change is recorded. This
is the scientific method as a ledningssystem (Manifesto #10, Feedback Cycle).

## muster is glue, not a second ledger (v1)

A control or check can **point at the authoritative source** instead of copying a
value into muster's own store (Manifesto #7, Single Source of Truth). A
reference-backed control derives its `title`/`status` by **resolving the ref on
read** — edit the source and muster reflects it on the next read, with no muster
mutation. The honest states `unresolved`/`stale` are surfaced, never silent green,
and a ref-backed control can **never show implemented/green when its resolved
source says fail**. v0's hand-set path still works, but is surfaced as
`asserted (unverified)`.

```bash
# point a control at a source — title/status resolve from the file on read
muster control add arch --title placeholder \
  --ref-file conformance-mapping.toml --ref-anchor requirements.CKSPEC-ARCH-001.title
# import many requirements as references (not copies) in one command
muster control import conformance-mapping.toml --prefix requirements
# one requirement, many implementations — each derives its own status (N:M)
muster control add-implementation arch --impl-id rust --ref-file report.json --ref-anchor a.b.status
```

## Install

muster is a single static binary, no runtime, no database.

```bash
# From git (works today — names the `muster` package in the workspace):
cargo install --git https://github.com/peiman/muster muster

# Or build the repo and put the binary on your PATH:
git clone https://github.com/peiman/muster && cd muster
cargo build --release          # → target/release/muster

# From crates.io — once published:
# cargo install muster
```

Verify: `muster version`. Requires a recent stable Rust toolchain (see
`rust-toolchain.toml`).

## Quick start

```bash
muster init                                   # zero-config; you are working
muster process add incident-mgmt --name "Incident Management" --owner ciso
muster control add a5-24 --title "Incident planning" --clause-ref "ISO 27001 A.5.24"
muster process link-control incident-mgmt a5-24
muster readiness                              # the honest truth-meter
muster explain                                # intent -> command map (no manual)
```

Every command supports `--output json` whose fields mirror the human text exactly
(dual surface, one source of truth). Exit codes are honest; errors name the fix.

## Configuration (environment)

muster is zero-config; these env knobs only tune honesty/freshness policy:

| Variable | Default | Effect |
|---|---|---|
| `MUSTER_DATA_DIR` | `./.muster` | Where the file-per-entity store lives. |
| `MUSTER_FRESHNESS_SECS` | `86400` | Freshness of a cached **command** *verdict* (only relevant with `MUSTER_CMD_CACHE` on) — past it the served verdict projects `stale`. `0` ⇒ never trust a cache. |
| `MUSTER_CMD_CACHE` | off | Opt in to serving cached command-ref verdicts (for genuinely expensive commands). The honest default re-resolves command refs **live** on every read. When on, `readiness` and `control resolve --all` **warn** that verdicts may be stale. |
| `MUSTER_SOURCE_FRESHNESS_SECS` | unset | A *different axis*: the age of a `file_anchor`'s pointed-at **source artifact** (its mtime), not a verdict. Past the bound the control is flagged `ref_source_stale` and held back from coverage — a confident `met` can't hide a file nobody regenerated. Unset or non-positive ⇒ no source-age gating. |

Two honesty rules worth knowing up front:

- **A bare number is not a verdict.** A control pointing at a metric (e.g.
  `coverage.percent`) stays `unknown` until you give it an expectation
  (`--expect ">=80"`); muster won't guess whether higher or lower is "good".
- **A note proves nothing.** A hand-set control counts toward READY only with a
  *verifying* artifact (`file`/`url`) — a `note` is honor-level and surfaces a
  `control_honor_evidence` gap until you attach real evidence or point a ref.

## Examples

- [`examples/ckeletin-feedback/`](examples/ckeletin-feedback/) — a worked,
  runnable example: governing a spec's feedback cycle (intake → triage → decide
  → implement → verify), with glue controls that read the real register and each
  consumer's live conformance report. The best five-minute tour of the whole
  idea — and it shows muster as an *optional* live view over a process that also
  enforces standalone in CI.
- [`examples/audit/`](examples/audit/) — an ISO 27001 audit scope where every
  control is wired at the **real evidence** it claims (a coverage metric with a
  bar, a backup-drill report, a live secrets scan, a hand-set policy control).
  `muster readiness` becomes the honest pre-audit truth-meter, `control show`
  resolves live so there's no stale green at audit time, and the bars also
  enforce in CI without muster.

## Design

Built on the [ckeletin-rust](https://github.com/peiman/ckeletin-rust) framework with
strict layering (`crates/{domain,infrastructure,cli}`): the domain is pure (entities,
graph traversal, readiness, the `Ref`/`Resolution` glue types), infrastructure owns
the dereference engine (the fs/process boundary — it cannot even see domain), and
the cli bridges the two and owns the disk boundary (file-per-entity JSON,
git-diffable, no database). `just check` is the conformance gate.

Run `muster explain` for the full command surface, or `muster <cmd> --help`.
