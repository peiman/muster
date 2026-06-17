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
graph traversal, readiness), the cli owns the disk boundary (file-per-entity JSON,
git-diffable, no database). `just check` is the conformance gate.

Run `muster explain` for the full command surface, or `muster <cmd> --help`.
