//! `readiness` — the headline value: a truth meter, not a checklist.
//!
//! Pure function over the `Store`. It reports certification-readiness honestly:
//! it distinguishes **proven** from merely **asserted** (#1 Truth-Seeking),
//! surfaces **refuting signals** that put a process-hypothesis under threat
//! (#2 Curiosity), records **enforcement strength** on the #9 ladder, lists
//! **guide-don't-gate gap findings** (#4), detects **graph cycles** (terminates,
//! never hangs), and emits a top-line verdict that is **never green while a gap
//! exists** (#3 honest signals).

use crate::model::{ControlStatus, IncidentStatus, NonconformityStatus, ProcessStatus};
use crate::reference::Derived;
use crate::store::Store;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Readiness {
    pub verdict: String,
    pub control_coverage: ControlCoverage,
    /// v1: controls split by whether their status is DERIVED from a ref (resolved
    /// from source) or merely ASSERTED (hand-set, unverified).
    pub controls: ControlsSplit,
    pub proven: Vec<String>,
    pub asserted: Vec<String>,
    pub refuting_signals: Vec<RefutingSignal>,
    pub enforcement: Vec<EnforcementEntry>,
    pub gap_findings: Vec<GapFinding>,
    pub cycles: Vec<Vec<String>>,
}

/// The v1 honesty split: a control whose status is resolved from an authoritative
/// source (`derived`) vs one hand-set without a ref (`asserted`, unverified).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ControlsSplit {
    pub derived: Vec<String>,
    pub asserted: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ControlCoverage {
    pub applicable: usize,
    pub implemented_with_evidence: usize,
    pub percent: f64,
    pub gaps: Vec<CoverageGap>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CoverageGap {
    pub id: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RefutingSignal {
    pub process_id: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EnforcementEntry {
    pub process_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strongest: Option<String>,
    /// `honor_only` or `no_enforcement` when the #9 ladder says "strengthen this".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flag: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GapFinding {
    pub kind: String,
    pub subject_id: String,
    pub message: String,
}

/// Compute readiness over the whole store, or (with `scope`) over one process
/// and its reachable sub-graph. v0 entry point — treats every control as asserted
/// (no resolution index); the cli calls `readiness_with` to pass derived
/// projections.
pub fn readiness(store: &Store, scope: Option<&str>) -> Readiness {
    readiness_with(store, scope, &BTreeMap::new(), None)
}

/// The v1 truth-meter. `index` maps a control id to its honest `Derived`
/// projection (built by the cli, which owns the clock + resolution — keeping the
/// domain clock- and I/O-free, #8). Controls absent from the index (or mapped to
/// `Asserted`) follow the v0 hand-set path; ref-backed controls count as covered
/// only when freshly `Derived` + `Pass`, and `Unresolved`/`Stale`/derived-`Fail`
/// become honest gap findings (never silent green, #1/#3).
///
/// `source_freshness_secs` is the opt-in source-age bound (b2): when `Some(b)`, a
/// live, passing `file_anchor` whose pointed-at artifact is older than `b`
/// seconds is held back as a coverage gap + a `ref_source_stale` finding (a
/// confident `met` must not hide a file nobody regenerated). `None` ⇒ no
/// source-age gating (today's behavior). The cli reads it from
/// `MUSTER_SOURCE_FRESHNESS_SECS`.
pub fn readiness_with(
    store: &Store,
    scope: Option<&str>,
    index: &BTreeMap<String, Derived>,
    source_freshness_secs: Option<i64>,
) -> Readiness {
    let in_scope: BTreeSet<String> = match scope {
        Some(id) => store.reachable(id),
        None => store.processes.keys().cloned().collect(),
    };

    let control_coverage = control_coverage(store, scope, &in_scope, index, source_freshness_secs);
    let controls = controls_split(store, scope, &in_scope, index);
    let (proven, asserted) = proven_vs_asserted(store, &in_scope);
    let refuting_signals = refuting_signals(store, &in_scope);
    let enforcement = enforcement(store, &in_scope);
    let cycles = scoped_cycles(store, scope, &in_scope);
    let gap_findings = gap_findings(
        store,
        scope,
        &in_scope,
        &cycles,
        index,
        source_freshness_secs,
    );

    // Full coverage via exact integer equality (no float comparison): every
    // applicable control is implemented-with-evidence.
    let full_coverage = control_coverage.implemented_with_evidence == control_coverage.applicable;
    // The headline counts EVERY readiness blocker, not just nonconformity
    // `gap_findings`. A coverage shortfall or a refuting signal blocks READY too,
    // so omitting them rendered "GAPS: 0" while a gap blocked readiness (dogfood
    // 2026-06-19). Coverage uses the exact integer shortfall — the same quantity
    // `full_coverage` tests — so when not READY this is provably ≥ 1 and the
    // headline can never under-report a block as "GAPS: 0".
    let blocking_gaps = gap_findings.len()
        + refuting_signals.len()
        + control_coverage
            .applicable
            .saturating_sub(control_coverage.implemented_with_evidence);
    let verdict = if gap_findings.is_empty() && refuting_signals.is_empty() && full_coverage {
        "READY".to_string()
    } else {
        format!("GAPS: {blocking_gaps}")
    };

    Readiness {
        verdict,
        control_coverage,
        controls,
        proven,
        asserted,
        refuting_signals,
        enforcement,
        gap_findings,
        cycles,
    }
}

/// A control is `derived` when its projection is anything other than `Asserted`
/// (i.e. it has a ref or implementations resolved from a source).
fn is_derived(index: &BTreeMap<String, Derived>, id: &str) -> bool {
    !matches!(index.get(id), None | Some(Derived::Asserted))
}

/// Split the in-scope controls into derived (resolved from source) vs asserted
/// (hand-set, unverified). id-sorted (deterministic).
fn controls_split(
    store: &Store,
    scope: Option<&str>,
    in_scope: &BTreeSet<String>,
    index: &BTreeMap<String, Derived>,
) -> ControlsSplit {
    let scoped = |id: &str| scope.is_none() || applicable_in_scope(store, scope, in_scope, id);
    let mut derived = Vec::new();
    let mut asserted = Vec::new();
    for c in store.controls.values() {
        if !scoped(&c.id) {
            continue;
        }
        if is_derived(index, &c.id) {
            derived.push(c.id.clone());
        } else {
            asserted.push(c.id.clone());
        }
    }
    ControlsSplit { derived, asserted }
}

/// The applicable controls relevant to this view. Unscoped: every applicable
/// control. Scoped: applicable controls referenced by the sub-graph
/// (`process.controls[]` ∪ each `step.controls[]`).
fn applicable_controls(
    store: &Store,
    scope: Option<&str>,
    in_scope: &BTreeSet<String>,
) -> Vec<String> {
    if scope.is_none() {
        return store
            .controls
            .values()
            .filter(|c| c.applicable)
            .map(|c| c.id.clone())
            .collect();
    }
    let mut referenced = BTreeSet::new();
    for pid in in_scope {
        if let Some(p) = store.processes.get(pid) {
            for c in &p.controls {
                referenced.insert(c.clone());
            }
            for s in &p.steps {
                for c in &s.controls {
                    referenced.insert(c.clone());
                }
            }
        }
    }
    referenced
        .into_iter()
        .filter(|id| store.controls.get(id).is_some_and(|c| c.applicable))
        .collect()
}

fn control_coverage(
    store: &Store,
    scope: Option<&str>,
    in_scope: &BTreeSet<String>,
    index: &BTreeMap<String, Derived>,
    source_freshness_secs: Option<i64>,
) -> ControlCoverage {
    let applicable_ids = applicable_controls(store, scope, in_scope);
    let applicable = applicable_ids.len();
    let mut implemented_with_evidence = 0usize;
    let mut gaps = Vec::new();
    for id in &applicable_ids {
        let c = match store.controls.get(id) {
            Some(c) => c,
            None => continue,
        };
        // A ref-backed control's resolution IS its evidence (#7): it counts as
        // covered only when freshly Derived + Pass. An asserted control follows
        // the v0 rule (status Implemented + attached evidence).
        if is_derived(index, id) {
            let d = index.get(id).expect("is_derived ⇒ present");
            // Stale-by-source: a live, passing file_anchor whose pointed-at
            // artifact is older than the (opt-in) source-freshness bound is NOT
            // fresh coverage — the verdict is live but derives from an
            // un-regenerated file (#1, b2). `None` bound ⇒ this never bites.
            let green = d.is_green_eligible();
            let stale_source = green && d.source_is_stale(source_freshness_secs);
            if green && !stale_source {
                implemented_with_evidence += 1;
            } else {
                let reason = if stale_source {
                    source_stale_reason(d)
                } else {
                    derived_gap_reason(d)
                };
                gaps.push(CoverageGap {
                    id: id.clone(),
                    reason,
                });
            }
            continue;
        }
        // STRICT honor path (v3.1): a note-only assertion is honor-level and does
        // NOT prove coverage; at least one *verifying* artifact (file/url) is
        // required (#1, symmetric with a note ref → Asserted, never green).
        let has_verifying = crate::model::has_verifying_evidence(&c.evidence);
        if c.status == ControlStatus::Implemented && has_verifying {
            implemented_with_evidence += 1;
        } else {
            let reason = match (c.status, c.evidence.is_empty(), has_verifying) {
                (ControlStatus::Implemented, true, _) => {
                    "implemented but has no evidence".to_string()
                }
                (ControlStatus::Implemented, false, false) => {
                    "implemented but evidence is honor-level (note only) — attach a file/url artifact or point a ref".to_string()
                }
                (status, _, _) => format!("status is {status}, not implemented-with-evidence"),
            };
            gaps.push(CoverageGap {
                id: id.clone(),
                reason,
            });
        }
    }
    // Counts are tiny (entity counts), far below f64's 2^52 exact-integer range,
    // so the precision-loss lint does not apply in practice.
    #[allow(clippy::cast_precision_loss)]
    let percent = if applicable == 0 {
        100.0
    } else {
        implemented_with_evidence as f64 / applicable as f64 * 100.0
    };
    ControlCoverage {
        applicable,
        implemented_with_evidence,
        percent,
        gaps,
    }
}

/// Is an incident "open" (a live refuting signal)? Anything not Closed.
fn incident_open(status: IncidentStatus) -> bool {
    status != IncidentStatus::Closed
}
/// Is a nonconformity "open"? Anything not Closed.
fn nc_open(status: NonconformityStatus) -> bool {
    status != NonconformityStatus::Closed
}

fn proven_vs_asserted(store: &Store, in_scope: &BTreeSet<String>) -> (Vec<String>, Vec<String>) {
    let mut proven = Vec::new();
    let mut asserted = Vec::new();
    for pid in in_scope {
        let p = match store.processes.get(pid) {
            Some(p) => p,
            None => continue,
        };
        if p.status != ProcessStatus::Active {
            continue;
        }
        // Honor path (v3.1): a note-only assertion does not prove a process —
        // proven requires a verifying artifact (file/url), not just "trust me".
        let has_verifying = crate::model::has_verifying_evidence(&p.evidence);
        let open_incident = store
            .incidents
            .values()
            .any(|i| i.process_ref.as_deref() == Some(pid) && incident_open(i.status));
        let open_nc = store
            .nonconformities
            .values()
            .any(|n| n.process_ref.as_deref() == Some(pid) && nc_open(n.status));
        let failed_check = p
            .checks
            .iter()
            .any(|c| c.last_result == crate::model::CheckResult::Fail);
        if has_verifying && !open_incident && !open_nc && !failed_check {
            proven.push(pid.clone());
        } else {
            asserted.push(pid.clone());
        }
    }
    (proven, asserted)
}

fn refuting_signals(store: &Store, in_scope: &BTreeSet<String>) -> Vec<RefutingSignal> {
    let mut out = Vec::new();
    for pid in in_scope {
        // open incidents
        for i in store.incidents.values() {
            if i.process_ref.as_deref() == Some(pid) && incident_open(i.status) {
                out.push(RefutingSignal {
                    process_id: pid.clone(),
                    source: format!("open incident {} ({})", i.id, i.status),
                });
            }
        }
        // open nonconformities
        for n in store.nonconformities.values() {
            if n.process_ref.as_deref() == Some(pid) && nc_open(n.status) {
                out.push(RefutingSignal {
                    process_id: pid.clone(),
                    source: format!("open nonconformity {} ({})", n.id, n.status),
                });
            }
        }
        // failed checks
        if let Some(p) = store.processes.get(pid) {
            for c in &p.checks {
                if c.last_result == crate::model::CheckResult::Fail {
                    out.push(RefutingSignal {
                        process_id: pid.clone(),
                        source: format!("failed check {}", c.id),
                    });
                }
            }
        }
    }
    out
}

fn enforcement(store: &Store, in_scope: &BTreeSet<String>) -> Vec<EnforcementEntry> {
    let mut out = Vec::new();
    for pid in in_scope {
        let p = match store.processes.get(pid) {
            Some(p) => p,
            None => continue,
        };
        match p.strongest_enforcement() {
            None => out.push(EnforcementEntry {
                process_id: pid.clone(),
                strongest: None,
                flag: Some("no_enforcement".to_string()),
            }),
            Some(e) => {
                let flag =
                    (e == crate::model::Enforcement::Honor).then(|| "honor_only".to_string());
                out.push(EnforcementEntry {
                    process_id: pid.clone(),
                    strongest: Some(e.to_string()),
                    flag,
                });
            }
        }
    }
    out
}

fn scoped_cycles(
    store: &Store,
    scope: Option<&str>,
    in_scope: &BTreeSet<String>,
) -> Vec<Vec<String>> {
    let all = store.detect_cycles();
    match scope {
        None => all,
        Some(_) => all
            .into_iter()
            .filter(|cyc| cyc.iter().any(|n| in_scope.contains(n)))
            .collect(),
    }
}

/// Human-readable reason a live, passing file_anchor is held back as a coverage
/// gap because its SOURCE artifact is stale-by-policy (b2). Distinct from the
/// cache `Stale` reason — the verdict resolved live; the artifact is old.
fn source_stale_reason(d: &Derived) -> String {
    format!(
        "resolved live but the source artifact is {}s old (exceeds the source-freshness bound) — regenerate it",
        d.source_age_secs().unwrap_or(0)
    )
}

/// Human-readable reason a ref-backed control is not covered.
fn derived_gap_reason(d: &Derived) -> String {
    match d {
        Derived::Unresolved { reason } => format!("ref unresolved: {reason}"),
        Derived::Stale { resolved_ts, .. } => {
            format!("ref resolution is stale (resolved {resolved_ts}) — re-resolve")
        }
        Derived::Derived { value, .. } => {
            format!("resolved source is not passing (value: {value})")
        }
        Derived::Asserted => "asserted (unverified)".to_string(),
    }
}

fn gap_findings(
    store: &Store,
    scope: Option<&str>,
    in_scope: &BTreeSet<String>,
    cycles: &[Vec<String>],
    index: &BTreeMap<String, Derived>,
    source_freshness_secs: Option<i64>,
) -> Vec<GapFinding> {
    let mut out = Vec::new();
    // Reference gaps (v1): a ref that can't be followed (unresolved), a cache
    // past its freshness bound (stale), or a source that resolves to fail —
    // each an honest gap, never silent green (#1/#3).
    for c in store.controls.values() {
        if !(scope.is_none() || applicable_in_scope(store, scope, in_scope, &c.id)) {
            continue;
        }
        match index.get(&c.id) {
            Some(Derived::Unresolved { reason }) => out.push(GapFinding {
                kind: "ref_unresolved".into(),
                subject_id: c.id.clone(),
                message: format!(
                    "control '{}' has a dangling ref — {reason}; fix the source or re-point: muster control show {}",
                    c.id, c.id
                ),
            }),
            Some(Derived::Stale { resolved_ts, .. }) => out.push(GapFinding {
                kind: "ref_stale".into(),
                subject_id: c.id.clone(),
                message: format!(
                    "control '{}' resolution is stale (resolved {resolved_ts}) — re-resolve: muster control resolve {}",
                    c.id, c.id
                ),
            }),
            Some(Derived::Derived {
                outcome: crate::reference::Outcome::Fail,
                value,
                ..
            }) => out.push(GapFinding {
                kind: "ref_failing".into(),
                subject_id: c.id.clone(),
                message: format!(
                    "control '{}' resolved source is failing (value: {value}) — fix the source",
                    c.id
                ),
            }),
            _ => {}
        }
        // Stale-by-source (b2): a live, passing file_anchor whose source artifact
        // is older than the opt-in bound — the verdict is live but the file is
        // un-regenerated. Independent of the match above (only fires on an
        // otherwise-green projection). `None` bound ⇒ never fires.
        if let Some(d) = index
            .get(&c.id)
            .filter(|d| d.is_green_eligible() && d.source_is_stale(source_freshness_secs))
        {
            out.push(GapFinding {
                kind: "ref_source_stale".into(),
                subject_id: c.id.clone(),
                message: format!(
                    "control '{}' resolved live but its source artifact is {}s old (exceeds the source-freshness bound) — regenerate the source, then: muster control resolve {}",
                    c.id,
                    d.source_age_secs().unwrap_or(0),
                    c.id
                ),
            });
        }
    }
    // Active processes: missing risks / metrics / controls (guide, don't gate).
    for pid in in_scope {
        let p = match store.processes.get(pid) {
            Some(p) => p,
            None => continue,
        };
        if p.status != ProcessStatus::Active {
            continue;
        }
        if p.risks.is_empty() {
            out.push(GapFinding {
                kind: "process_no_risks".into(),
                subject_id: pid.clone(),
                message: format!("active process '{pid}' has no risks — add: muster process risk add {pid} \"<risk>\""),
            });
        }
        if p.metrics.is_empty() {
            out.push(GapFinding {
                kind: "process_no_metrics".into(),
                subject_id: pid.clone(),
                message: format!("active process '{pid}' has no metrics — add: muster process metric add {pid} \"<metric>\""),
            });
        }
        if p.controls.is_empty() {
            out.push(GapFinding {
                kind: "process_no_controls".into(),
                subject_id: pid.clone(),
                message: format!("active process '{pid}' has no controls — link: muster process link-control {pid} <control-id>"),
            });
        }
    }
    // Controls implemented but with no — or only honor-level — evidence (in
    // scope). An empty-evidence control is `control_no_evidence`; a note-only
    // control is `control_honor_evidence` (v3.1 strict honor path: a note proves
    // nothing — name the verifying-artifact fix).
    let scoped_control =
        |id: &str| -> bool { scope.is_none() || applicable_in_scope(store, scope, in_scope, id) };
    for c in store.controls.values() {
        if c.status != ControlStatus::Implemented || !scoped_control(&c.id) {
            continue;
        }
        if c.evidence.is_empty() {
            out.push(GapFinding {
                kind: "control_no_evidence".into(),
                subject_id: c.id.clone(),
                message: format!("control '{}' is implemented but has no evidence — attach: muster control attach-evidence {} <kind> <value>", c.id, c.id),
            });
        } else if !crate::model::has_verifying_evidence(&c.evidence) {
            out.push(GapFinding {
                kind: "control_honor_evidence".into(),
                subject_id: c.id.clone(),
                message: format!("control '{}' is implemented but its only evidence is a note (honor-level) — attach a file/url artifact: muster control attach-evidence {} file <path>", c.id, c.id),
            });
        }
    }
    // Open nonconformities with no corrective action.
    for n in store.nonconformities.values() {
        let relevant = scope.is_none()
            || n.process_ref
                .as_deref()
                .is_some_and(|p| in_scope.contains(p));
        if relevant && nc_open(n.status) && n.corrective_action.as_deref().unwrap_or("").is_empty()
        {
            out.push(GapFinding {
                kind: "nonconformity_no_corrective_action".into(),
                subject_id: n.id.clone(),
                message: format!("open nonconformity '{}' has no corrective action — set: muster nonconformity resolve {} --corrective-action \"<action>\"", n.id, n.id),
            });
        }
    }
    // Each cycle is a finding.
    for cyc in cycles {
        out.push(GapFinding {
            kind: "cycle".into(),
            subject_id: cyc.join(" -> "),
            message: format!("process graph cycle: {} — break the loop", cyc.join(" -> ")),
        });
    }
    out
}

fn applicable_in_scope(
    store: &Store,
    scope: Option<&str>,
    in_scope: &BTreeSet<String>,
    id: &str,
) -> bool {
    if scope.is_none() {
        return true;
    }
    in_scope.iter().any(|pid| {
        store.processes.get(pid).is_some_and(|p| {
            p.controls.iter().any(|c| c == id)
                || p.steps.iter().any(|s| s.controls.iter().any(|c| c == id))
        })
    })
}

impl fmt::Display for Readiness {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "readiness: {}", self.verdict)?;
        writeln!(
            f,
            "  control coverage: {}/{} applicable implemented-with-evidence ({:.0}%)",
            self.control_coverage.implemented_with_evidence,
            self.control_coverage.applicable,
            self.control_coverage.percent
        )?;
        for g in &self.control_coverage.gaps {
            writeln!(f, "    gap: {} — {}", g.id, g.reason)?;
        }
        writeln!(
            f,
            "  controls (derived): {}",
            join_or_none(&self.controls.derived)
        )?;
        writeln!(
            f,
            "  controls (asserted, unverified): {}",
            join_or_none(&self.controls.asserted)
        )?;
        writeln!(f, "  proven: {}", join_or_none(&self.proven))?;
        writeln!(f, "  asserted: {}", join_or_none(&self.asserted))?;
        if !self.refuting_signals.is_empty() {
            writeln!(f, "  refuting signals:")?;
            for r in &self.refuting_signals {
                writeln!(f, "    {} — {}", r.process_id, r.source)?;
            }
        }
        if !self.enforcement.is_empty() {
            writeln!(f, "  enforcement:")?;
            for e in &self.enforcement {
                let strongest = e.strongest.as_deref().unwrap_or("none");
                let flag = e
                    .flag
                    .as_ref()
                    .map(|fl| format!(" [{fl}]"))
                    .unwrap_or_default();
                writeln!(f, "    {} — {}{}", e.process_id, strongest, flag)?;
            }
        }
        if !self.gap_findings.is_empty() {
            writeln!(f, "  gap findings:")?;
            for g in &self.gap_findings {
                writeln!(f, "    [{}] {}", g.kind, g.message)?;
            }
        }
        if !self.cycles.is_empty() {
            writeln!(f, "  cycles:")?;
            for c in &self.cycles {
                writeln!(f, "    {}", c.join(" -> "))?;
            }
        }
        Ok(())
    }
}

fn join_or_none(items: &[String]) -> String {
    if items.is_empty() {
        "(none)".to_string()
    } else {
        items.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    fn base() -> Store {
        let mut s = Store::default();
        s.add_process("p1", "P1", None, None).unwrap();
        s
    }

    #[test]
    fn zero_applicable_controls_is_100_percent() {
        let s = base();
        let r = readiness(&s, None);
        assert_eq!(r.control_coverage.percent, 100.0);
    }

    #[test]
    fn coverage_math_and_gap() {
        let mut s = base();
        s.add_control("c1", "C1", None, true).unwrap();
        s.set_control_status("c1", ControlStatus::Implemented)
            .unwrap();
        s.attach_control_evidence(
            "c1",
            Evidence {
                kind: EvidenceKind::File, // verifying artifact (note-only no longer counts)
                value: "x".into(),
            },
        )
        .unwrap();
        s.add_control("c2", "C2", None, true).unwrap(); // applicable, no evidence
        let r = readiness(&s, None);
        assert_eq!(r.control_coverage.applicable, 2);
        assert_eq!(r.control_coverage.implemented_with_evidence, 1);
        assert_eq!(r.control_coverage.percent, 50.0);
        assert!(r.control_coverage.gaps.iter().any(|g| g.id == "c2"));
    }

    #[test]
    fn note_only_evidence_does_not_satisfy_coverage() {
        // STRICT honor path (v3.1): a hand-set control marked Implemented whose
        // ONLY evidence is a note is honor-level — it must NOT count as
        // implemented-with-evidence (symmetric with a note *ref* → Asserted,
        // never green). It is a coverage gap until a file/url artifact (or a ref)
        // is attached, and surfaces a `control_honor_evidence` finding naming the fix.
        let mut s = base();
        s.add_control("c1", "C1", None, true).unwrap();
        s.set_control_status("c1", ControlStatus::Implemented)
            .unwrap();
        s.attach_control_evidence(
            "c1",
            Evidence {
                kind: EvidenceKind::Note,
                value: "did it".into(),
            },
        )
        .unwrap();
        let r = readiness(&s, None);
        assert_eq!(
            r.control_coverage.implemented_with_evidence, 0,
            "note-only is honor-level, not verified evidence"
        );
        assert!(r.control_coverage.gaps.iter().any(|g| g.id == "c1"));
        assert!(
            r.gap_findings
                .iter()
                .any(|g| g.kind == "control_honor_evidence" && g.subject_id == "c1"),
            "an honor-evidence finding must name the fix"
        );
    }

    #[test]
    fn file_evidence_satisfies_coverage() {
        // The honor path only bites notes: a verifying artifact (file/url) counts.
        let mut s = base();
        s.add_control("c1", "C1", None, true).unwrap();
        s.set_control_status("c1", ControlStatus::Implemented)
            .unwrap();
        s.attach_control_evidence(
            "c1",
            Evidence {
                kind: EvidenceKind::File,
                value: "report.json".into(),
            },
        )
        .unwrap();
        let r = readiness(&s, None);
        assert_eq!(r.control_coverage.implemented_with_evidence, 1);
        assert!(
            !r.gap_findings
                .iter()
                .any(|g| g.kind == "control_honor_evidence"),
            "a file artifact is verifying — no honor finding"
        );
    }

    #[test]
    fn source_stale_file_anchor_is_a_gap_only_when_bound_set() {
        use crate::reference::{Derived, Outcome};
        let mut s = base();
        s.add_control("c1", "C1", None, true).unwrap();
        // a live-resolved, passing file_anchor whose SOURCE artifact is 1000s old.
        let stale_src = Derived::Derived {
            value: "met".into(),
            outcome: Outcome::Pass,
            resolved_ts: "t".into(),
            source_excerpt: None,
            resolved_age_secs: 0,
            served_from_cache: false,
            source_age_secs: Some(1000),
        };
        let mut index = BTreeMap::new();
        index.insert("c1".to_string(), stale_src);

        // opt-in: no source-freshness bound ⇒ counts as covered (today's behavior).
        let r0 = readiness_with(&s, None, &index, None);
        assert_eq!(r0.control_coverage.implemented_with_evidence, 1);
        assert!(
            !r0.gap_findings.iter().any(|g| g.kind == "ref_source_stale"),
            "no bound ⇒ no source-stale gating"
        );

        // bound 600 < 1000 age ⇒ stale-by-source: not fresh coverage, a finding.
        let r = readiness_with(&s, None, &index, Some(600));
        assert_eq!(
            r.control_coverage.implemented_with_evidence, 0,
            "a stale source is not fresh coverage"
        );
        assert!(r.control_coverage.gaps.iter().any(|g| g.id == "c1"));
        assert!(
            r.gap_findings
                .iter()
                .any(|g| g.kind == "ref_source_stale" && g.subject_id == "c1"),
            "a source-stale finding must name the control"
        );
        assert_ne!(r.verdict, "READY");
    }

    #[test]
    fn failing_source_that_is_also_stale_yields_ref_failing_not_double_fire() {
        // The ref_source_stale arm is gated on is_green_eligible(), so a control
        // whose source FAILS (and is also old) reports `ref_failing` only — it
        // must NOT also emit `ref_source_stale` (no double-flag on the same
        // control across two axes). The Fail already blocks readiness.
        use crate::reference::{Derived, Outcome};
        let mut s = base();
        s.add_control("c1", "C1", None, true).unwrap();
        let failing_and_old = Derived::Derived {
            value: "0".into(),
            outcome: Outcome::Fail,
            resolved_ts: "t".into(),
            source_excerpt: None,
            resolved_age_secs: 0,
            served_from_cache: false,
            source_age_secs: Some(1000),
        };
        let mut index = BTreeMap::new();
        index.insert("c1".to_string(), failing_and_old);

        let r = readiness_with(&s, None, &index, Some(600));
        assert!(
            r.gap_findings.iter().any(|g| g.kind == "ref_failing"),
            "a failing source is a ref_failing gap"
        );
        assert!(
            !r.gap_findings.iter().any(|g| g.kind == "ref_source_stale"),
            "a non-green (failing) projection must NOT also emit ref_source_stale"
        );
    }

    #[test]
    fn note_only_process_evidence_is_asserted_not_proven() {
        let mut s = base();
        s.set_process_status("p1", ProcessStatus::Active).unwrap();
        s.attach_process_evidence(
            "p1",
            Evidence {
                kind: EvidenceKind::Note,
                value: "trust me".into(),
            },
        )
        .unwrap();
        assert!(
            readiness(&s, None).asserted.contains(&"p1".to_string()),
            "note-only process evidence ⇒ asserted, not proven"
        );
        // a verifying artifact promotes it to proven.
        s.attach_process_evidence(
            "p1",
            Evidence {
                kind: EvidenceKind::File,
                value: "log.txt".into(),
            },
        )
        .unwrap();
        assert!(readiness(&s, None).proven.contains(&"p1".to_string()));
    }

    #[test]
    fn proven_requires_evidence_and_no_refuting() {
        let mut s = base();
        s.set_process_status("p1", ProcessStatus::Active).unwrap();
        // no evidence yet -> asserted
        assert!(readiness(&s, None).asserted.contains(&"p1".to_string()));
        s.attach_process_evidence(
            "p1",
            Evidence {
                kind: EvidenceKind::File, // verifying artifact (note-only no longer proves)
                value: "v".into(),
            },
        )
        .unwrap();
        assert!(readiness(&s, None).proven.contains(&"p1".to_string()));
        // open incident refutes
        s.report_incident("inc-1", "O", Severity::High, Some("p1".into()))
            .unwrap();
        let r = readiness(&s, None);
        assert!(r.asserted.contains(&"p1".to_string()));
        assert!(r.refuting_signals.iter().any(|x| x.process_id == "p1"));
    }

    #[test]
    fn resolving_signals_removes_them_and_moves_numbers() {
        let mut s = base();
        s.set_process_status("p1", ProcessStatus::Active).unwrap();
        s.report_incident("inc-1", "O", Severity::High, Some("p1".into()))
            .unwrap();
        s.raise_nonconformity(
            "nc-1",
            "slow",
            NonconformitySource::Manual,
            Some("inc-1".into()),
            None,
            None,
        )
        .unwrap();
        let before = readiness(&s, None);
        assert!(!before.refuting_signals.is_empty());
        s.resolve_nonconformity("nc-1", Some("fixed".into()))
            .unwrap();
        s.close_incident("inc-1").unwrap();
        let after = readiness(&s, None);
        assert!(
            !after
                .refuting_signals
                .iter()
                .any(|r| r.source.contains("nc-1") || r.source.contains("inc-1"))
        );
        // SC-9 delta: refuting_signals count dropped.
        assert!(after.refuting_signals.len() < before.refuting_signals.len());
    }

    #[test]
    fn enforcement_reports_strongest_on_ladder() {
        let mut s = base();
        let c = s.add_check("p1", "d", Enforcement::Ci).unwrap();
        s.ingest_check("p1", &c, CheckResult::Pass, "t", None)
            .unwrap();
        let r = readiness(&s, None);
        let e = r.enforcement.iter().find(|e| e.process_id == "p1").unwrap();
        assert_eq!(e.strongest.as_deref(), Some("ci"));
        assert!(e.flag.is_none());
    }

    #[test]
    fn verdict_never_green_with_a_gap() {
        let mut s = base();
        s.set_process_status("p1", ProcessStatus::Active).unwrap(); // active -> no risks/metrics/controls gaps
        let r = readiness(&s, None);
        assert!(r.verdict.starts_with("GAPS:"));
        assert_eq!(r.verdict, format!("GAPS: {}", r.gap_findings.len()));
    }

    #[test]
    fn verdict_counts_coverage_gaps_not_just_gap_findings() {
        // THE headline-honesty hole (dogfood 2026-06-19): a control that is
        // applicable but NOT implemented-with-evidence is a coverage gap that
        // blocks READY — but it produces NO `gap_finding`, so the old count
        // (`gap_findings.len()`) rendered "GAPS: 0" while a gap blocked
        // readiness. The headline must count EVERY blocker: a coverage shortfall
        // makes the count ≥ 1, never "GAPS: 0" while not READY.
        let mut s = base();
        s.add_risk("p1", "r").unwrap();
        s.add_metric("p1", "m").unwrap();
        // Applicable control, linked, but NOT implemented-with-evidence → a
        // coverage gap with zero gap_findings.
        s.add_control("c1", "C", None, true).unwrap();
        s.link_control("p1", "c1").unwrap();
        s.set_process_status("p1", ProcessStatus::Active).unwrap();
        let r = readiness(&s, None);
        assert_eq!(
            r.gap_findings.len(),
            0,
            "no nonconformity gap_findings here"
        );
        assert!(
            !r.control_coverage.gaps.is_empty(),
            "the uncovered control IS a coverage gap"
        );
        assert_ne!(
            r.verdict, "GAPS: 0",
            "the headline must NOT read GAPS: 0 while a coverage gap blocks readiness"
        );
        assert_ne!(r.verdict, "READY");
        // The count includes the coverage shortfall.
        let shortfall =
            r.control_coverage.applicable - r.control_coverage.implemented_with_evidence;
        assert_eq!(
            r.verdict,
            format!("GAPS: {}", r.gap_findings.len() + shortfall)
        );
    }

    #[test]
    fn verdict_ready_only_at_zero_gaps_and_full_coverage() {
        let mut s = base();
        // active, fully-specified process; covered control.
        s.add_risk("p1", "r").unwrap();
        s.add_metric("p1", "m").unwrap();
        s.add_control("c1", "C", None, true).unwrap();
        s.link_control("p1", "c1").unwrap();
        s.set_control_status("c1", ControlStatus::Implemented)
            .unwrap();
        s.attach_control_evidence(
            "c1",
            Evidence {
                kind: EvidenceKind::File, // verifying artifact (note-only no longer counts)
                value: "x".into(),
            },
        )
        .unwrap();
        s.set_process_status("p1", ProcessStatus::Active).unwrap();
        let r = readiness(&s, None);
        assert_eq!(r.verdict, "READY", "unexpected gaps: {:?}", r.gap_findings);
    }

    #[test]
    fn cycle_is_reported_in_gap_findings_and_cycles() {
        let mut s = base();
        s.add_process("p2", "P2", None, None).unwrap();
        s.add_step("p1", "", None, vec![], Some("p2".into()))
            .unwrap();
        s.add_step("p2", "", None, vec![], Some("p1".into()))
            .unwrap();
        let r = readiness(&s, None);
        assert!(!r.cycles.is_empty());
        assert!(r.gap_findings.iter().any(|g| g.kind == "cycle"));
    }
}
