#!/bin/sh
# Enforce the feedback process in CI WITHOUT muster — proving muster is an
# optional view, not a dependency. Exits non-zero (fails the build) if the
# register has anything untriaged or undecided, or if any consumer broke
# conformance. This is the source-of-truth enforcement; `muster readiness` is
# the same truth rendered as a living process map.
set -e
cd "$(dirname "$0")"

echo "feedback: no report left untriaged ..."
sh checks/no_open_feedback.sh

echo "feedback: every closed report has a decision ..."
sh checks/decisions_recorded.sh

echo "feedback: every consumer still conforms to the spec ..."
for report in consumers/*/conformance-report.json; do
  consumer=$(basename "$(dirname "$report")")
  passed=$(python3 -c "import json,sys; print(json.load(open(sys.argv[1]))['summary']['passed'])" "$report")
  if [ "$passed" != "True" ]; then
    echo "  consumer '$consumer' no longer conforms ($report: passed=$passed)" >&2
    exit 1
  fi
  echo "  $consumer: ok"
done

echo "feedback process: OK"
