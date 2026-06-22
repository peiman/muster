#!/usr/bin/env bash
# muster verifies muster — the recursive dogfood.
#
# The v3 feature is "done" exactly when muster's OWN readiness verdict, over a
# control wired at the live exit code of the acceptance script, is READY. This is
# the thesis in miniature: a deterministic, evidence-anchored verdict is what tells
# you the work is actually achieved — so we let muster render that verdict about its
# own construction. Exits 0 (READY) iff acceptance/roundtrip.sh passes (#9, #10).
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO"
cargo build --release -p muster >/dev/null 2>&1 || { echo "FAIL: build"; exit 1; }
BIN="$REPO/target/release/muster"

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
export MUSTER_DATA_DIR="$WORK/store"

"$BIN" init >/dev/null
"$BIN" process add ship-v3 --name "Ship muster v3 round-trip" --owner peiman >/dev/null
"$BIN" process risk add ship-v3 "round-trip is not a fixpoint" >/dev/null
"$BIN" process metric add ship-v3 "acceptance criteria passing" >/dev/null
# The control's truth IS the live exit code of the black-box acceptance script —
# resolved live on every read (v2 default), so it can never show a stale green (#9).
"$BIN" control add acceptance --title "v3 round-trip acceptance" \
  --ref-cmd "bash $REPO/acceptance/roundtrip.sh" --ref-dir "$REPO" >/dev/null
"$BIN" process link-control ship-v3 acceptance >/dev/null

echo "muster's verdict on its own v3 construction:"
"$BIN" readiness --require-ready
