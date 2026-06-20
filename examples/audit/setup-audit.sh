#!/bin/sh
# Stand up an ISO 27001 audit scope in muster, with every control wired at the
# REAL evidence it claims — so `muster readiness` is an honest pre-audit
# truth-meter and `muster control show` resolves live at audit time (no stale
# green). Requires `muster` on PATH (see the repo README → Install). Run from
# this dir. Re-runnable after `rm -rf .muster`.
set -e
cd "$(dirname "$0")"

muster init

# The spine: the ISMS as a process (a hypothesis about how security is run),
# with one risk + one metric so it is well-formed, then set active.
muster process add isms --name "Information Security Management System" --owner ciso
muster process risk   add isms "loss of confidentiality/integrity/availability of customer data"
muster process metric add isms "% of Annex A controls implemented-with-evidence"
muster process set-status isms active

# Controls mapped to Annex A clauses, each wired at a REAL source. muster derives
# Pass/Fail by RESOLVING that source on every read — never by a value copied here.

# A.8.29 — Security testing: line coverage must be >= 80% (a metric + a bar).
muster control add a8-29 --title "Security testing in development" \
  --clause-ref "ISO 27001 A.8.29" \
  --ref-file evidence/coverage.json --ref-anchor summary.line_percent --expect ">=80"

# A.8.13 — Information backup: the last quarterly restore drill passed (a verdict).
muster control add a8-13 --title "Information backup (restore drill)" \
  --clause-ref "ISO 27001 A.8.13" \
  --ref-report evidence/backup-drill.json last_restore.status

# A.5.18 — Access rights: reviewed within the last 90 days (a metric + a bar).
muster control add a5-18 --title "Access rights review" \
  --clause-ref "ISO 27001 A.5.18" \
  --ref-file evidence/access-review.json --ref-anchor review.days_since --expect "<=90"

# A.8.12 — Data leakage: a LIVE secrets scan (command; exit 0 = pass, re-run each read).
muster control add a8-12 --title "Data leakage prevention (secrets scan)" \
  --clause-ref "ISO 27001 A.8.12" \
  --ref-cmd "sh checks/secrets_scan.sh" --ref-dir .

# A.5.1 — Policies: hand-set (no automated source yet) — muster surfaces it as
# ASSERTED (unverified) and will NOT count it toward readiness until you attach a
# VERIFYING artifact (file/url). A `note` is honor-level and still won't count.
# This is the audit-grade honesty: hand-waving never reads green.
muster control add a5-1 --title "Information security policy" \
  --clause-ref "ISO 27001 A.5.1"

# Govern the ISMS with the controls.
for c in a8-29 a8-13 a5-18 a8-12 a5-1; do
  muster process link-control isms "$c"
done

echo
echo "Audit scope defined. Now:  muster readiness"
