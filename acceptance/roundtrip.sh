#!/usr/bin/env bash
# Black-box acceptance for muster v3 — the declarative round-trip (`state` / `apply`).
#
# Drives ONLY the built `muster` binary (no internal knowledge of the JSON schema:
# every "differ" is a string-replace on real `state` output, so the test is robust
# to whatever serde shape the implementation chooses). Exits 0 iff every SPEC v3
# Definition-of-Done criterion holds.
#
# Committed RED on purpose: `state`/`apply` do not exist yet, so this fails today.
# Making it pass is the goal of the run. Do NOT delete or weaken it (#9).
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO"

cargo build --release -p muster >/dev/null 2>&1 || { echo "FAIL: cargo build -p muster"; exit 1; }
BIN="$REPO/target/release/muster"
[ -x "$BIN" ] || { echo "FAIL: binary not found at $BIN"; exit 1; }

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
export MUSTER_DATA_DIR="$WORK/store"

# A resolvable file_anchor source. Absolute path → cwd-independent resolution.
FIX="$WORK/coverage.json"
printf '{"coverage": {"percent": 92}}\n' > "$FIX"

fail() { echo "FAIL: $1"; exit 1; }
m() { "$BIN" "$@"; }

# ── Seed a store with today's per-entity commands. This N+1 dance IS the AX pain
#    v3 removes; the acceptance below proves the one-shot round-trip that replaces it.
m init >/dev/null
m process add ship-roundtrip --name "Ship the round-trip" --owner peiman >/dev/null
m process risk add ship-roundtrip "manifest drift" >/dev/null
m process metric add ship-roundtrip "acceptance-pass-rate" >/dev/null
m control add cov-bar --title "coverage bar" \
  --ref-file "$FIX" --ref-anchor coverage.percent --expect ">=80" >/dev/null
m process link-control ship-roundtrip cov-bar >/dev/null

# ── 1. Full read — `state --output json` emits every entity (not a verdict view).
S1="$WORK/s1.json"
m state --output json > "$S1" || fail "(1) 'muster state --output json' failed"
grep -q "ship-roundtrip" "$S1" || fail "(1) state output missing the process"
grep -q "cov-bar"        "$S1" || fail "(1) state output missing the control"

# ── 2. Round-trip is a fixpoint: state -> wipe -> apply -> state == state.
rm -rf "$MUSTER_DATA_DIR"
m init >/dev/null
m apply "$S1" >/dev/null || fail "(2) 'muster apply' of captured state failed"
S2="$WORK/s2.json"
m state --output json > "$S2"
diff "$S1" "$S2" >/dev/null || fail "(2) round-trip is not a fixpoint: state != apply(state)"

# ── 3. Idempotent apply — applying the same manifest twice changes nothing.
m apply "$S1" >/dev/null || fail "(3) second apply failed"
S3="$WORK/s3.json"
m state --output json > "$S3"
diff "$S1" "$S3" >/dev/null || fail "(3) apply is not idempotent"

# ── 4. --dry-run mutates nothing, yet prints the would-be readiness verdict.
#    The changed manifest differs only in a free-text value (string-replace, no
#    schema assumptions) so it is a VALID manifest that WOULD change the store.
CHANGED="$WORK/changed.json"
sed 's/Ship the round-trip/MUTATED-by-dry-run/g' "$S1" > "$CHANGED"
DRY="$("$BIN" apply --dry-run "$CHANGED" 2>&1)" || fail "(4) 'apply --dry-run' exited non-zero"
echo "$DRY" | grep -qiE "ready|gaps|verdict" || fail "(4) dry-run did not print a readiness verdict"
S4="$WORK/s4.json"
m state --output json > "$S4"
diff "$S1" "$S4" >/dev/null || fail "(4) --dry-run mutated the store"

# ── 5. Fail-closed — a manifest with a dangling file_anchor anchor is refused as a
#    whole; the store is left exactly as it was (all-or-nothing). The manifest keeps
#    the EXACT serde shape (only the anchor string is broken via replace).
BAD="$WORK/bad.json"
sed 's/coverage\.percent/coverage.DOES_NOT_EXIST/g' "$S1" > "$BAD"
if "$BIN" apply "$BAD" >/dev/null 2>&1; then
  fail "(5) apply of a dangling-anchor manifest should fail-closed, but it succeeded"
fi
S5="$WORK/s5.json"
m state --output json > "$S5"
diff "$S1" "$S5" >/dev/null || fail "(5) a failed apply mutated the store (not all-or-nothing)"

# ── 6. Discoverability — explain + catalog both list the two new verbs.
m explain 2>&1 | grep -q "state" || fail "(6) 'muster explain' omits 'state'"
m explain 2>&1 | grep -q "apply" || fail "(6) 'muster explain' omits 'apply'"
CAT="$(m catalog --output json 2>&1)"
echo "$CAT" | grep -q "state" || fail "(6) 'muster catalog' omits 'state'"
echo "$CAT" | grep -q "apply" || fail "(6) 'muster catalog' omits 'apply'"

# ── 7. Trust boundary — a DUPLICATE id within a category is refused as a whole; the
#    store is left unchanged. `apply` is adversarial input, not a trusted dump: the
#    interactive path rejects duplicate ids, so apply must too (last-write-wins is a
#    silent corruption). Schema-agnostic: duplicate the first control object verbatim.
DUP="$WORK/dup.json"
python3 - "$S1" "$DUP" <<'PY'
import json, sys
d = json.load(open(sys.argv[1]))
doc = d["data"] if isinstance(d.get("data"), dict) and "controls" in d["data"] else d
ctrls = doc["controls"]
assert isinstance(ctrls, list) and ctrls, "expected a non-empty controls array in state output"
ctrls.append(json.loads(json.dumps(ctrls[0])))  # an exact-id duplicate
json.dump(d, open(sys.argv[2], "w"))
PY
if "$BIN" apply "$DUP" >/dev/null 2>&1; then
  fail "(7) apply of a duplicate-id manifest should fail-closed, but it succeeded"
fi
S7="$WORK/s7.json"; m state --output json > "$S7"
diff "$S1" "$S7" >/dev/null || fail "(7) a refused duplicate-id apply mutated the store"

# ── 8. Trust boundary — an UNKNOWN entity field is refused, never silently dropped
#    (a misspelled key must be an honest error so "what it reads it can write back
#    exactly" cannot quietly lose data). Inject a bogus field into a real entity.
UNK="$WORK/unknown.json"
python3 - "$S1" "$UNK" <<'PY'
import json, sys
d = json.load(open(sys.argv[1]))
doc = d["data"] if isinstance(d.get("data"), dict) and "controls" in d["data"] else d
doc["controls"][0]["bogus_unknown_field"] = True
json.dump(d, open(sys.argv[2], "w"))
PY
if "$BIN" apply "$UNK" >/dev/null 2>&1; then
  fail "(8) apply of an unknown-field manifest should fail-closed, but it succeeded"
fi
S8="$WORK/s8.json"; m state --output json > "$S8"
diff "$S1" "$S8" >/dev/null || fail "(8) a refused unknown-field apply mutated the store"

echo "PASS: muster v3 declarative round-trip — all 8 acceptance criteria hold."
