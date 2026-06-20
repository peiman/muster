#!/bin/sh
# Pass (exit 0) iff no non-open report has a null decision (no silent drops).
# Uses python3 (stdlib) for a robust JSON read; falls back gracefully.
register="${1:-feedback-register.json}"
python3 - "$register" <<'PY'
import json, sys
items = json.load(open(sys.argv[1]))["items"]
bad = [i["id"] for i in items if i.get("status") != "open" and not i.get("decision")]
if bad:
    sys.stderr.write("closed report(s) with no decision: %s\n" % ", ".join(bad))
    sys.exit(1)
PY
