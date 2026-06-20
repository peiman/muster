#!/bin/sh
# Define the ckeletin spec-feedback process in muster (the live view).
# Requires `muster` on PATH (see the repo README → Install). Run from this dir.
# Re-runnable after `rm -rf .muster`.
set -e
cd "$(dirname "$0")"

muster init

# The process — the spine (a hypothesis about how feedback flows).
muster process add ckeletin-feedback \
  --name "ckeletin spec feedback cycle" --owner peiman

# The steps — intake -> triage -> DECIDE -> implement -> verify.
muster process step add ckeletin-feedback --description "intake: log every report in the register"
muster process step add ckeletin-feedback --description "triage: real gap, misunderstanding, or out-of-scope?"
muster process step add ckeletin-feedback --description "decide: UPDATE the spec / DISCUSS with the reporter / REJECT (with a reason)"
muster process step add ckeletin-feedback --description "implement: bump the spec + regenerate"
muster process step add ckeletin-feedback --description "verify: consumers re-conform; the loop closes"

# The glue controls — each resolves a REAL source on every read, so the
# readiness verdict can never be greener than reality.
muster control add c-triaged   --title "no report left untriaged" \
  --ref-cmd "sh checks/no_open_feedback.sh" --ref-dir .
muster control add c-decided   --title "no report silently dropped" \
  --ref-cmd "sh checks/decisions_recorded.sh" --ref-dir .
muster control add c-muster    --title "consumer muster still conforms" \
  --ref-file consumers/muster/conformance-report.json --ref-anchor summary.passed
muster control add c-workhorse --title "consumer workhorse still conforms" \
  --ref-file consumers/workhorse/conformance-report.json --ref-anchor summary.passed

# Govern the process with the controls.
for c in c-triaged c-decided c-muster c-workhorse; do
  muster process link-control ckeletin-feedback "$c"
done

echo
echo "Process defined. Now:  muster readiness"
