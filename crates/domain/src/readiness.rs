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
use crate::store::Store;
use serde::Serialize;
use std::collections::BTreeSet;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Readiness {
    pub verdict: String,
    pub control_coverage: ControlCoverage,
    pub proven: Vec<String>,
    pub asserted: Vec<String>,
    pub refuting_signals: Vec<RefutingSignal>,
    pub enforcement: Vec<EnforcementEntry>,
    pub gap_findings: Vec<GapFinding>,
    pub cycles: Vec<Vec<String>>,
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
/// and its reachable sub-graph.
pub fn readiness(store: &Store, scope: Option<&str>) -> Readiness {
    let in_scope: BTreeSet<String> = match scope {
        Some(id) => store.reachable(id),
        None => store.processes.keys().cloned().collect(),
    };

    let control_coverage = control_coverage(store, scope, &in_scope);
    let (proven, asserted) = proven_vs_asserted(store, &in_scope);
    let refuting_signals = refuting_signals(store, &in_scope);
    let enforcement = enforcement(store, &in_scope);
    let cycles = scoped_cycles(store, scope, &in_scope);
    let gap_findings = gap_findings(store, scope, &in_scope, &cycles);

    let verdict = if gap_findings.is_empty()
        && refuting_signals.is_empty()
        && control_coverage.percent == 100.0
    {
        "READY".to_string()
    } else {
        format!("GAPS: {}", gap_findings.len())
    };

    Readiness {
        verdict,
        control_coverage,
        proven,
        asserted,
        refuting_signals,
        enforcement,
        gap_findings,
        cycles,
    }
}

/// The applicable controls relevant to this view. Unscoped: every applicable
/// control. Scoped: applicable controls referenced by the sub-graph
/// (`process.controls[]` ∪ each `step.controls[]`).
fn applicable_controls(store: &Store, scope: Option<&str>, in_scope: &BTreeSet<String>) -> Vec<String> {
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

fn control_coverage(store: &Store, scope: Option<&str>, in_scope: &BTreeSet<String>) -> ControlCoverage {
    let applicable_ids = applicable_controls(store, scope, in_scope);
    let applicable = applicable_ids.len();
    let mut implemented_with_evidence = 0usize;
    let mut gaps = Vec::new();
    for id in &applicable_ids {
        let c = match store.controls.get(id) {
            Some(c) => c,
            None => continue,
        };
        let has_evidence = !c.evidence.is_empty();
        if c.status == ControlStatus::Implemented && has_evidence {
            implemented_with_evidence += 1;
        } else {
            let reason = match (c.status, has_evidence) {
                (ControlStatus::Implemented, false) => "implemented but has no evidence".to_string(),
                (status, _) => format!("status is {status}, not implemented-with-evidence"),
            };
            gaps.push(CoverageGap {
                id: id.clone(),
                reason,
            });
        }
    }
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
        let has_evidence = !p.evidence.is_empty();
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
        if has_evidence && !open_incident && !open_nc && !failed_check {
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
                let flag = (e == crate::model::Enforcement::Honor).then(|| "honor_only".to_string());
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

fn scoped_cycles(store: &Store, scope: Option<&str>, in_scope: &BTreeSet<String>) -> Vec<Vec<String>> {
    let all = store.detect_cycles();
    match scope {
        None => all,
        Some(_) => all
            .into_iter()
            .filter(|cyc| cyc.iter().any(|n| in_scope.contains(n)))
            .collect(),
    }
}

fn gap_findings(
    store: &Store,
    scope: Option<&str>,
    in_scope: &BTreeSet<String>,
    cycles: &[Vec<String>],
) -> Vec<GapFinding> {
    let mut out = Vec::new();
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
    // Controls implemented but evidence-less (in scope).
    let scoped_control =
        |id: &str| -> bool { scope.is_none() || applicable_in_scope(store, scope, in_scope, id) };
    for c in store.controls.values() {
        if c.status == ControlStatus::Implemented && c.evidence.is_empty() && scoped_control(&c.id) {
            out.push(GapFinding {
                kind: "control_no_evidence".into(),
                subject_id: c.id.clone(),
                message: format!("control '{}' is implemented but has no evidence — attach: muster control attach-evidence {} <kind> <value>", c.id, c.id),
            });
        }
    }
    // Open nonconformities with no corrective action.
    for n in store.nonconformities.values() {
        let relevant = scope.is_none()
            || n.process_ref.as_deref().is_some_and(|p| in_scope.contains(p));
        if relevant
            && nc_open(n.status)
            && n.corrective_action.as_deref().unwrap_or("").is_empty()
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

fn applicable_in_scope(store: &Store, scope: Option<&str>, in_scope: &BTreeSet<String>, id: &str) -> bool {
    if scope.is_none() {
        return true;
    }
    in_scope.iter().any(|pid| {
        store.processes.get(pid).is_some_and(|p| {
            p.controls.iter().any(|c| c == id) || p.steps.iter().any(|s| s.controls.iter().any(|c| c == id))
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
                let flag = e.flag.as_ref().map(|fl| format!(" [{fl}]")).unwrap_or_default();
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
        s.set_control_status("c1", ControlStatus::Implemented).unwrap();
        s.attach_control_evidence("c1", Evidence { kind: EvidenceKind::Note, value: "x".into() }).unwrap();
        s.add_control("c2", "C2", None, true).unwrap(); // applicable, no evidence
        let r = readiness(&s, None);
        assert_eq!(r.control_coverage.applicable, 2);
        assert_eq!(r.control_coverage.implemented_with_evidence, 1);
        assert_eq!(r.control_coverage.percent, 50.0);
        assert!(r.control_coverage.gaps.iter().any(|g| g.id == "c2"));
    }

    #[test]
    fn proven_requires_evidence_and_no_refuting() {
        let mut s = base();
        s.set_process_status("p1", ProcessStatus::Active).unwrap();
        // no evidence yet -> asserted
        assert!(readiness(&s, None).asserted.contains(&"p1".to_string()));
        s.attach_process_evidence("p1", Evidence { kind: EvidenceKind::Note, value: "v".into() }).unwrap();
        assert!(readiness(&s, None).proven.contains(&"p1".to_string()));
        // open incident refutes
        s.report_incident("inc-1", "O", Severity::High, Some("p1".into())).unwrap();
        let r = readiness(&s, None);
        assert!(r.asserted.contains(&"p1".to_string()));
        assert!(r.refuting_signals.iter().any(|x| x.process_id == "p1"));
    }

    #[test]
    fn resolving_signals_removes_them_and_moves_numbers() {
        let mut s = base();
        s.set_process_status("p1", ProcessStatus::Active).unwrap();
        s.report_incident("inc-1", "O", Severity::High, Some("p1".into())).unwrap();
        s.raise_nonconformity("nc-1", "slow", NonconformitySource::Manual, Some("inc-1".into()), None, None).unwrap();
        let before = readiness(&s, None);
        assert!(!before.refuting_signals.is_empty());
        s.resolve_nonconformity("nc-1", Some("fixed".into())).unwrap();
        s.close_incident("inc-1").unwrap();
        let after = readiness(&s, None);
        assert!(!after.refuting_signals.iter().any(|r| r.source.contains("nc-1") || r.source.contains("inc-1")));
        // SC-9 delta: refuting_signals count dropped.
        assert!(after.refuting_signals.len() < before.refuting_signals.len());
    }

    #[test]
    fn enforcement_reports_strongest_on_ladder() {
        let mut s = base();
        let c = s.add_check("p1", "d", Enforcement::Ci).unwrap();
        s.ingest_check("p1", &c, CheckResult::Pass, "t", None).unwrap();
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
    fn verdict_ready_only_at_zero_gaps_and_full_coverage() {
        let mut s = base();
        // active, fully-specified process; covered control.
        s.add_risk("p1", "r").unwrap();
        s.add_metric("p1", "m").unwrap();
        s.add_control("c1", "C", None, true).unwrap();
        s.link_control("p1", "c1").unwrap();
        s.set_control_status("c1", ControlStatus::Implemented).unwrap();
        s.attach_control_evidence("c1", Evidence { kind: EvidenceKind::Note, value: "x".into() }).unwrap();
        s.set_process_status("p1", ProcessStatus::Active).unwrap();
        let r = readiness(&s, None);
        assert_eq!(r.verdict, "READY", "unexpected gaps: {:?}", r.gap_findings);
    }

    #[test]
    fn cycle_is_reported_in_gap_findings_and_cycles() {
        let mut s = base();
        s.add_process("p2", "P2", None, None).unwrap();
        s.add_step("p1", "", None, vec![], Some("p2".into())).unwrap();
        s.add_step("p2", "", None, vec![], Some("p1".into())).unwrap();
        let r = readiness(&s, None);
        assert!(!r.cycles.is_empty());
        assert!(r.gap_findings.iter().any(|g| g.kind == "cycle"));
    }
}
