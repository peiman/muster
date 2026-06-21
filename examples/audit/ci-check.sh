#!/bin/sh
# Enforce audit-readiness in CI WITHOUT muster — proving muster is an optional
# *view*, not a dependency. Exits non-zero (fails the build) if any control's
# real evidence no longer meets its bar. `muster readiness` is the same truth
# rendered as a living, clause-mapped process map. For the muster-native CI gate
# (same truth, one primitive), use: `muster readiness --require-ready` (exits 3
# on gaps, 0 when READY).
set -e
cd "$(dirname "$0")"

j() { python3 -c "import json,sys;print(json.load(open(sys.argv[1]))$2)" "$1"; }

echo "A.8.29 security testing: line coverage >= 80% ..."
cov=$(j evidence/coverage.json "['summary']['line_percent']")
awk "BEGIN{exit !($cov >= 80)}" || { echo "  coverage $cov% < 80%" >&2; exit 1; }
echo "  $cov% ok"

echo "A.8.13 information backup: last restore drill met ..."
st=$(j evidence/backup-drill.json "['last_restore']['status']")
[ "$st" = "met" ] || { echo "  last restore drill is '$st', not met" >&2; exit 1; }
echo "  met ok"

echo "A.5.18 access rights: reviewed within 90 days ..."
days=$(j evidence/access-review.json "['review']['days_since']")
awk "BEGIN{exit !($days <= 90)}" || { echo "  last review $days days ago (> 90)" >&2; exit 1; }
echo "  $days days ok"

echo "A.8.12 data leakage: secrets scan clean ..."
sh checks/secrets_scan.sh && echo "  clean ok"

echo "audit scope: OK"
