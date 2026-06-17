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

## Design

Built on the [ckeletin-rust](https://github.com/peiman/ckeletin-rust) framework with
strict layering (`crates/{domain,infrastructure,cli}`): the domain is pure (entities,
graph traversal, readiness, the `Ref`/`Resolution` glue types), infrastructure owns
the dereference engine (the fs/process boundary — it cannot even see domain), and
the cli bridges the two and owns the disk boundary (file-per-entity JSON,
git-diffable, no database). `just check` is the conformance gate.

Run `muster explain` for the full command surface, or `muster <cmd> --help`.
