#!/bin/sh
# Pass (exit 0) iff no feedback item is still untriaged.
# Source of truth: feedback-register.json. No tools required (grep only).
register="${1:-feedback-register.json}"
if grep -q '"status": *"open"' "$register"; then
  echo "untriaged report(s) still open in $register" >&2
  exit 1
fi
exit 0
