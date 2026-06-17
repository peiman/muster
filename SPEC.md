# muster — SPEC (values brief)

> *پندارِ نیک، گفتارِ نیک، کردارِ نیک* — good thoughts, good words, good deeds.
> *Skynda långsamt.*

**muster** is a minimalistic, AI-first command-line **ledningssystem** (management
system) for **startups and midsize companies with no compliance department**. It
lets them run their management system as a living **process map**, become
**ISO-certification-ready** without consultants, and handle **incidents / command
& control** — from one small, honest tool, usable by AI agents and humans alike.

The name carries the thesis: *muster* the team and resources (incident C2) **and**
*pass muster* (meet the standard). Two jobs, one spine.

## The core idea — a process is a hypothesis

This tool is the management-system expression of Manifesto principle #10
(Feedback Cycle): **"A specification is a hypothesis; implementations test it. When
better approaches emerge, the specification evolves."**

A process in muster is **not true because someone wrote it down.** It is a
*hypothesis* about how work should be done. Reality tests it — through evidence
(#1 Truth-Seeking), through failures that are signals not just defects
(#2 Curiosity Over Certainty), and through automated conformance checks
(#9 Automated Enforcement). When reality refutes or outgrows a process, **the
process changes**, and that change is recorded. Later, CI and automation systems
will drive muster to *really follow* the processes (the seam for that is built in
v0; the CI plugin itself is later — #5 Platforms, Not Features).

So muster is not a static compliance recorder. It is a **living, falsifiable,
evidence-driven** process system: the scientific method as a ledningssystem.

## Who it is for

Startups and midsize companies with **no dedicated compliance team**. They must
become certifiable (ISO 9001 / 14001 / 27001, etc.) and manage incidents, but
cannot afford ceremony or consultants. muster works on day one with zero config
and **guides** the user toward readiness rather than demanding it.

## Principles (these define the product; each is bound to the Manifesto)

1. **AX-first, but humans too.** Every command serves an AI agent driving it *and*
   a human reading it — two renderings of one truth, not two modes of thought.
2. **Dual surface, one source of truth (#7 SSOT).** Every command supports
   `--output json` whose fields mirror **all** the data structurally — never a
   human-only string, never markdown stuffed in a JSON blob. The default rendering
   is clean human-readable text. Text and JSON tell the *same* story.
3. **Minimal ceremony (#4 Lean Iteration).** `muster init` and you are working. No
   mandatory config; the only required fields are identity + a description.
   Sensible defaults. A founder gets value in five minutes.
4. **Guide, do not gate (#2).** muster never blocks you for being incomplete.
   `readiness` *illuminates* gaps (a process with no risks, a control with no
   evidence, an open nonconformity, an honor-system-only process). It nags toward
   certification; it never stands in the way of moving fast.
5. **Honest signals (#3 Good Will — build anchors).** Exit codes are truthful.
   `readiness` never paints green over an unmet standard, and distinguishes
   **proven** from merely **asserted**. Errors name what is wrong *and the fix*.
6. **Hand the next action.** After a command, muster suggests the natural next step
   — serving a cold agent and a new founder identically.
7. **Self-describing.** `muster explain` gives an intent-first map; every command
   has clear `--help`. No manual required.
8. **Standard-agnostic.** muster hard-codes no standard. You define the controls
   you must meet (any framework); muster manages them. Importing a named control
   set is a later feature, not a v0 dependency.

## Foundation

Scaffold from the **ckeletin-rust** framework at `/Users/peiman/dev/ckeletin-rust`
(the same framework workhorse consumes): layered `crates/{domain,infrastructure,
cli}`, the `.ckeletin/` conformance harness, `ckeletin-project.toml`, `just`
recipes. Keep `just check` green. Rust; no network, no database, no async runtime.

## Data model — Process is the spine

All entities are **JSON files** under a per-project data dir (git-diffable, no
database). `muster init` creates the store.

### Process (the spine; a node in a directed graph; a hypothesis)
```
id            slug, unique                              required
name                                                    required
purpose                                                 optional
owner         role or person                            optional
status        proposed | active | under_review | retired   default: proposed
inputs[]      strings                                   optional
outputs[]     strings                                   optional
steps[]       Step objects (below)                      optional
controls[]    control ids governing the whole process   optional
risks[]       strings        (readiness flags if empty on an active process)
metrics[]     strings        (readiness flags if empty on an active process)
checks[]      Check objects (below) — conformance signals, the CI seam
revisions[]   Revision objects (below) — the feedback cycle, append-only
evidence[]    Evidence refs                             optional
```
`status` is the **hypothesis lifecycle**: `proposed` (a hypothesis, unproven) →
`active` (in use) → `under_review` (reality is diverging) → `retired`. muster also
derives a **`proven`** signal = has validating evidence **and** no open
nonconformities/failed checks against it. `readiness` reports proven-vs-asserted.

**Step** (an ordered activity; recursion point):
```
n             integer order              required
description                              required
owner                                    optional
controls[]    control ids at this step   optional
process_ref   id of a sub-process        optional   ← processes compose recursively
```
A step with `process_ref` delegates to another process → the process map is a
graph. **Cycles are possible; muster detects them and reports them as a finding**
(never loops forever).

**Check** (a conformance signal — #9 Automated Enforcement; the CI seam):
```
id            slug                                       required
description                                             required
enforcement   compile_time | lint | script | ci | honor    required
last_result   pass | fail | unknown      default: unknown
last_run_ts                              set on ingest
evidence[]    Evidence refs             optional
```
`enforcement` encodes the #9 ladder. An `honor`-only check is the weakest form of
enforcement; `readiness` flags it as a gap to strengthen. Results are ingested via
`muster process check` (v0 = manual/scripted ingest; a CI plugin calls the same
seam later).

**Revision** (the feedback cycle made auditable — #10):
```
ts                                       set on write
summary       what changed and why       required
because       optional id of the incident/nonconformity/check that triggered it
```

### Control (standard-agnostic requirement)
```
id            slug, unique               required
title                                     required
clause_ref    free text (e.g. "ISO 27001 A.5.24")   optional
applicable    bool                        default: true
status        not_started | in_progress | implemented   default: not_started
evidence[]    Evidence refs              optional
```

### Incident (occurs within a process; a refuting signal)
```
id            slug, unique               required
title                                     required
severity      low | medium | high | critical   default: medium
status        open | mitigating | closed  default: open
process_ref   id of the process it occurred in   optional
log[]         timeline entries {ts, note}, appended via `incident log`
```

### Nonconformity (a finding against a process/control; a refuting signal)
```
id            slug, unique               required
source        incident | audit | manual   required
process_ref   id                          optional
control_ref   id                          optional
description                               required
corrective_action  free text             optional
status        open | in_progress | closed   default: open
```
Can be raised from an incident (`--from-incident <id>`), copying its process_ref.

### Evidence
A reference attached to a process / control / nonconformity / check:
`{ kind: file | url | note, value }`.

## Command surface (all dual-surface: human text by default, `--output json` mirrors all fields)

```
muster init
muster explain

muster process add <id> --name … [--owner … --purpose …]
muster process show <id> [--tree]
muster process list
muster process set-status <id> <proposed|active|under_review|retired>
muster process step add <id> --description … [--control … --owner … --process-ref …]
muster process link-control <id> <control-id>
muster process risk add <id> "<risk>"
muster process metric add <id> "<metric>"
muster process check add <id> --description … --enforcement <compile_time|lint|script|ci|honor>
muster process check <id> <check-id> --pass|--fail [--evidence <kind> <value>]   # ingest a conformance result
muster process revise <id> "<what changed>" [--because <incident|nonconformity|check id>]
muster process attach-evidence <id> <kind> <value>

muster control add <id> --title … [--clause-ref … --applicable true|false]
muster control list | show <id> | set-status <id> <status> | attach-evidence <id> <kind> <value>

muster incident report <id> --title … [--severity … --process <pid>]
muster incident list | show <id> | log <id> "<note>" | close <id>

muster nonconformity raise <id> --description … [--from-incident <iid> | --process <pid> | --control <cid>] [--source …]
muster nonconformity list | show <id> | resolve <id> [--corrective-action "…"]

muster readiness [--process <id>]
```

## `muster readiness` — the headline value (a truth meter, not a checklist)

Computes certification-readiness over the process graph, in text and JSON. Reports
at minimum:
- **Control coverage**: of applicable controls (process- + step-level, rolled up
  across the graph), how many are `implemented` *with at least one evidence* — a %
  and the gap list.
- **Proven vs. asserted**: which active processes are *proven* (validating
  evidence, no open refuting signals) vs. merely asserted.
- **Refuting signals**: processes carrying open incidents/nonconformities, or a
  failed last check — "hypothesis under threat, review it" (#2, #10).
- **Enforcement strength**: per process, the strongest enforcement among its
  checks; honor-only processes flagged as a gap to strengthen (#9 ladder).
- **Gap findings** (guide-don't-gate): active processes with no risks / no metrics
  / no controls; controls implemented but evidence-less; nonconformities with no
  corrective action; **process-graph cycles**.
- A single honest top-line verdict (e.g. `READY` / `GAPS: N`) that never reads
  green while gaps exist.

`--process <id>` scopes to one process and its sub-graph.

## AX / dual-surface conventions (non-negotiable acceptance criteria)

- Every command accepts `--output json`; the JSON contains every field the text
  shows (and more), as structured data — **no** human-only fields, **no** markdown
  or pre-rendered tables inside JSON.
- Exit codes are honest: `0` only on success; non-zero with a clear stderr message
  naming the fix.
- Not-found / invalid-input errors name the offending id/field **and** the
  corrective command.
- `muster explain` maps intents → commands.
- Output is deterministic (stable ordering) so agents can diff and assert.

## Definition of done — the validator grades this end-to-end

A reviewer with **no prior knowledge of muster**, using only `--help`/`explain`
and `--output json`, drives the full spine and observes truthful results:

1. `muster init`; `process add incident-mgmt --name "Incident Management" --owner
   ciso` → succeeds (status defaults to `proposed`); `process show … --output json`
   returns the structured process.
2. Create `containment`; add a step to `incident-mgmt` delegating to it
   (`--process-ref containment`); `process show incident-mgmt --tree` expands it.
3. `control add a5-24 --title "Incident planning" --clause-ref "ISO 27001 A.5.24"`;
   `process link-control incident-mgmt a5-24`; `control set-status a5-24
   implemented` + attach evidence.
4. `process check add incident-mgmt --description "runbook exists in CI" \
   --enforcement ci`; `process check incident-mgmt <check> --pass` → recorded.
5. `incident report inc-1 --title … --process incident-mgmt`; `incident log inc-1
   "contained"`.
6. `nonconformity raise nc-1 --from-incident inc-1 --description …`; then
   `process revise incident-mgmt "tightened detection step" --because nc-1`
   (the feedback cycle); `nonconformity resolve nc-1 --corrective-action …`.
7. `process set-status incident-mgmt active`; `readiness --output json` reflects
   all the above truthfully: control coverage %, proven-vs-asserted, zero open
   nonconformities after resolve, enforcement strength (the `ci` check), the
   remaining gap findings, and an honest top-line verdict. Re-running after a
   change moves the numbers correctly.
8. Introduce a process cycle and confirm `readiness` reports it rather than
   hanging.
9. Every command behaves identically in JSON and human modes (same facts), and
   `just check` is green.

## Out of scope for v0 (do NOT build)

No embedded LLM / AI provider (muster is *driven by* agents; it does not call one).
No network, database, web UI, auth/multi-tenant. No actual CI plugin (only the
conformance-ingest *seam*). No import of named standard control sets. No
PDF/report export. Minimalism is a feature, not a compromise.

---

## How to build this — the Manifesto (every phase: read and apply it)

This project is built by autonomous agents (planner, critic, executor, reviewer,
validator). **Every phase must read and develop by Peiman's Manifesto**
(github.com/peiman/manifesto). It is not decoration — the validator grades
manifesto-alignment (e.g., is a process honestly *proven-vs-asserted*? are
enforcement strengths recorded per #9? is the feedback cycle a first-class,
auditable artifact per #10?), not just feature presence.

The ten principles, verbatim:

1. **Truth-Seeking** — Observe, trace, verify. Every conclusion rests on evidence.
   Assumptions compound into drift; evidence prevents it.
2. **Curiosity Over Certainty** — When something fails, it signals something to
   understand, not merely fix. The failure is the lesson.
3. **Good Will** — Build robustness as the default. The bad must not outgrow the
   good. Build anchors.
4. **Lean Iteration** — Build the smallest thing producing real data. Run it.
   Reality is the specification — not imagination, but observation.
5. **Platforms, Not Features** — Each step is a platform for the next. Build heavy
   enough to support what comes after, clean enough that nothing rots underneath.
6. **Partnership** — Hold each other to high standards. Invest in growth, not just
   output.
7. **Single Source of Truth** — Every piece of information has one authoritative
   location. Reference the source rather than copying it.
8. **Separation of Concerns** — Different responsibilities live in different
   places. Separation enables independent evolution.
9. **Automated Enforcement** — Rules without enforcement erode. Prefer compile-time
   over linting, linting over scripts, scripts over CI, CI over honor systems.
10. **Feedback Cycle** — Specifications and implementations learn from each other.
    A specification is a hypothesis; implementations test it. When better
    approaches emerge, the specification evolves.

> *"The only constant in the fabric of existence is irony — and in the core of
> everything contradictory, beauty lies."* — P.K.
