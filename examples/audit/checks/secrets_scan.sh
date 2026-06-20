#!/bin/sh
# A live command control for ISO 27001 A.8.12 (data leakage prevention):
# exit 0 (pass) iff no obvious secret pattern is committed under ./evidence.
# A stand-in for your real scanner (gitleaks/trufflehog) — the point is muster
# RE-RUNS it on every read, so a secret introduced tomorrow flips the control red
# with no one remembering to re-check. No tools required (grep only).
set -e
cd "$(dirname "$0")/.."

# Patterns a real scanner would catch; kept deliberately narrow for the demo.
if grep -rEq '(AKIA[0-9A-Z]{16}|-----BEGIN [A-Z ]*PRIVATE KEY-----)' evidence/; then
  echo "secret-like material found under evidence/ — remediate before audit" >&2
  exit 1
fi
exit 0
