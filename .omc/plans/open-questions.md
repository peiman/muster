# muster v2 — open questions / deferred (not silently dropped)

## Downward issue/nonconformity propagation (P2) — DEFERRED
SPEC P2 / PLAN §2.6 / SC-7: a failing (Fail/Unresolved) control could set a
re-evaluation flag on the processes that link it, surfaced in `readiness` as a
`flagged_for_reeval` list. NOT built this iteration (P0/P1 + the other P2
residuals — SSOT cache drop, cycle-safety test, drift profile — were prioritized
to fully close the headline drift window). The honest signal already exists
indirectly: a failing ref-backed control linked to a process surfaces as a
`ref_failing` / `ref_unresolved` gap_finding and blocks the READY verdict, so a
linked process is never silently green. The dedicated downward `flagged_for_reeval`
list is the remaining nicety. Tracked here per Manifesto #10.

## Validation severity for bad anchors — CHOSEN: refuse (fail-closed, #9)
`control add`/`add-implementation` refuse a dangling file_anchor at store time
(SC-5). A future `--allow-dangling` opt-out could support point-then-create-source
workflows if it ever blocks legitimate use. `control import` intentionally does
NOT refuse (bulk ingest) — dangling imported anchors surface as readiness gaps and
via `control resolve --all`.

## Source-freshness ENFORCED bound (MUSTER_SOURCE_FRESHNESS_SECS) — DEFERRED
The required deliverable (visible `source_age_secs`) ships. An optional enforced
bound that flags a stale source in the drift profile is stretch, not built.
