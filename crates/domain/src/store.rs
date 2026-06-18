//! The in-memory aggregate and all pure operations over it.
//!
//! Every mutator validates (slug, uniqueness, referential existence, enum
//! membership) and is **total** — it returns `Result<_, DomainError>` and never
//! panics. Timestamps are passed *in* from the cli boundary so the domain stays
//! deterministic and clock-free (Manifesto #1, #8). `BTreeMap` keys keep all
//! iteration id-sorted → deterministic output an agent can diff (#7, AX).

use crate::model::*;
use crate::reference::{Ref, Resolution};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

/// The whole management system, in memory. Persisted file-per-entity by the cli
/// layer (the disk boundary lives there, not here — #8).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Store {
    pub processes: BTreeMap<String, Process>,
    pub controls: BTreeMap<String, Control>,
    pub incidents: BTreeMap<String, Incident>,
    pub nonconformities: BTreeMap<String, Nonconformity>,
}

const ADD_PROCESS_FIX: &str = "create it with: muster process add <id> --name <name>";
const ADD_CONTROL_FIX: &str = "create it with: muster control add <id> --title <title>";
const REPORT_INCIDENT_FIX: &str = "create it with: muster incident report <id> --title <title>";
const RAISE_NC_FIX: &str = "create it with: muster nonconformity raise <id> --description <desc>";

impl Store {
    // ── readers ──────────────────────────────────────────────────────────────

    pub fn process(&self, id: &str) -> Result<&Process, DomainError> {
        self.processes
            .get(id)
            .ok_or_else(|| DomainError::nf("process", id, ADD_PROCESS_FIX))
    }
    pub fn control(&self, id: &str) -> Result<&Control, DomainError> {
        self.controls
            .get(id)
            .ok_or_else(|| DomainError::nf("control", id, ADD_CONTROL_FIX))
    }
    pub fn incident(&self, id: &str) -> Result<&Incident, DomainError> {
        self.incidents
            .get(id)
            .ok_or_else(|| DomainError::nf("incident", id, REPORT_INCIDENT_FIX))
    }
    pub fn nonconformity(&self, id: &str) -> Result<&Nonconformity, DomainError> {
        self.nonconformities
            .get(id)
            .ok_or_else(|| DomainError::nf("nonconformity", id, RAISE_NC_FIX))
    }

    pub fn list_processes(&self) -> Vec<&Process> {
        self.processes.values().collect()
    }
    pub fn list_controls(&self) -> Vec<&Control> {
        self.controls.values().collect()
    }
    pub fn list_incidents(&self) -> Vec<&Incident> {
        self.incidents.values().collect()
    }
    pub fn list_nonconformities(&self) -> Vec<&Nonconformity> {
        self.nonconformities.values().collect()
    }

    fn require_process(&self, id: &str) -> Result<(), DomainError> {
        if self.processes.contains_key(id) {
            Ok(())
        } else {
            Err(DomainError::mref("process", id, ADD_PROCESS_FIX))
        }
    }
    fn require_control(&self, id: &str) -> Result<(), DomainError> {
        if self.controls.contains_key(id) {
            Ok(())
        } else {
            Err(DomainError::mref("control", id, ADD_CONTROL_FIX))
        }
    }

    /// Does this id name an incident, a nonconformity, or any check? (Used to
    /// validate `revise --because`, which can cite any refuting signal.)
    fn is_known_cause(&self, id: &str) -> bool {
        self.incidents.contains_key(id)
            || self.nonconformities.contains_key(id)
            || self
                .processes
                .values()
                .any(|p| p.checks.iter().any(|c| c.id == id))
    }

    // ── process mutators ─────────────────────────────────────────────────────

    pub fn add_process(
        &mut self,
        id: &str,
        name: &str,
        owner: Option<String>,
        purpose: Option<String>,
    ) -> Result<(), DomainError> {
        validate_slug(id)?;
        if self.processes.contains_key(id) {
            return Err(DomainError::DuplicateId {
                kind: "process",
                id: id.to_string(),
            });
        }
        let mut p = Process::new(id.to_string(), name.to_string());
        p.owner = owner;
        p.purpose = purpose;
        self.processes.insert(id.to_string(), p);
        Ok(())
    }

    pub fn set_process_status(
        &mut self,
        id: &str,
        status: ProcessStatus,
    ) -> Result<(), DomainError> {
        let p = self
            .processes
            .get_mut(id)
            .ok_or_else(|| DomainError::nf("process", id, ADD_PROCESS_FIX))?;
        p.status = status;
        Ok(())
    }

    pub fn add_step(
        &mut self,
        pid: &str,
        description: &str,
        owner: Option<String>,
        controls: Vec<String>,
        process_ref: Option<String>,
    ) -> Result<i64, DomainError> {
        self.require_process(pid)?;
        if let Some(r) = &process_ref {
            self.require_process(r)?;
        }
        for c in &controls {
            self.require_control(c)?;
        }
        let p = self.processes.get_mut(pid).expect("checked above");
        let n = p.steps.iter().map(|s| s.n).max().unwrap_or(0) + 1;
        p.steps.push(Step {
            n,
            description: description.to_string(),
            owner,
            controls,
            process_ref,
        });
        Ok(n)
    }

    pub fn link_control(&mut self, pid: &str, cid: &str) -> Result<(), DomainError> {
        self.require_process(pid)?;
        self.require_control(cid)?;
        let p = self.processes.get_mut(pid).expect("checked above");
        if !p.controls.iter().any(|c| c == cid) {
            p.controls.push(cid.to_string());
        }
        Ok(())
    }

    pub fn add_risk(&mut self, pid: &str, risk: &str) -> Result<(), DomainError> {
        let p = self
            .processes
            .get_mut(pid)
            .ok_or_else(|| DomainError::nf("process", pid, ADD_PROCESS_FIX))?;
        p.risks.push(risk.to_string());
        Ok(())
    }

    pub fn add_metric(&mut self, pid: &str, metric: &str) -> Result<(), DomainError> {
        let p = self
            .processes
            .get_mut(pid)
            .ok_or_else(|| DomainError::nf("process", pid, ADD_PROCESS_FIX))?;
        p.metrics.push(metric.to_string());
        Ok(())
    }

    /// Create a conformance check; returns the generated check id (`check-<n>`,
    /// deterministic per process).
    pub fn add_check(
        &mut self,
        pid: &str,
        description: &str,
        enforcement: Enforcement,
    ) -> Result<String, DomainError> {
        self.require_process(pid)?;
        let p = self.processes.get_mut(pid).expect("checked above");
        let n = p.checks.len() + 1;
        let id = format!("check-{n}");
        p.checks.push(Check {
            id: id.clone(),
            description: description.to_string(),
            enforcement,
            last_result: CheckResult::Unknown,
            last_run_ts: None,
            evidence: Vec::new(),
            r#ref: None,
            resolved: None,
        });
        Ok(id)
    }

    /// Ingest a conformance result — the #9 seam a CI plugin calls later.
    pub fn ingest_check(
        &mut self,
        pid: &str,
        check_id: &str,
        result: CheckResult,
        ts: &str,
        evidence: Option<Evidence>,
    ) -> Result<(), DomainError> {
        let p = self
            .processes
            .get_mut(pid)
            .ok_or_else(|| DomainError::nf("process", pid, ADD_PROCESS_FIX))?;
        let check = p
            .checks
            .iter_mut()
            .find(|c| c.id == check_id)
            .ok_or_else(|| {
                DomainError::nf(
                    "check",
                    check_id,
                    "list checks with: muster process show <id>",
                )
            })?;
        // Honesty rule (SC-5): a reference-backed check derives its result from
        // its source on read — it can NEVER be hand-set to forge a green.
        if check.is_ref_backed() {
            return Err(DomainError::RefBacked {
                kind: "check",
                id: check_id.to_string(),
                fix: "this check derives its result from its ref; fix the source, then re-resolve with: muster process check <pid> <check-id> --resolve".to_string(),
            });
        }
        check.last_result = result;
        check.last_run_ts = Some(ts.to_string());
        if let Some(e) = evidence {
            check.evidence.push(e);
        }
        Ok(())
    }

    /// Append a revision — the #10 feedback cycle, audit-grade and append-only.
    pub fn revise(
        &mut self,
        pid: &str,
        summary: &str,
        because: Option<String>,
        ts: &str,
    ) -> Result<(), DomainError> {
        self.require_process(pid)?;
        if let Some(b) = &because
            && !self.is_known_cause(b)
        {
            return Err(DomainError::mref(
                "cause",
                b,
                "--because must cite an existing incident, nonconformity, or check id",
            ));
        }
        let p = self.processes.get_mut(pid).expect("checked above");
        p.revisions.push(Revision {
            ts: ts.to_string(),
            summary: summary.to_string(),
            because,
        });
        Ok(())
    }

    pub fn attach_process_evidence(
        &mut self,
        pid: &str,
        evidence: Evidence,
    ) -> Result<(), DomainError> {
        let p = self
            .processes
            .get_mut(pid)
            .ok_or_else(|| DomainError::nf("process", pid, ADD_PROCESS_FIX))?;
        p.evidence.push(evidence);
        Ok(())
    }

    // ── control mutators ─────────────────────────────────────────────────────

    pub fn add_control(
        &mut self,
        id: &str,
        title: &str,
        clause_ref: Option<String>,
        applicable: bool,
    ) -> Result<(), DomainError> {
        validate_slug(id)?;
        if self.controls.contains_key(id) {
            return Err(DomainError::DuplicateId {
                kind: "control",
                id: id.to_string(),
            });
        }
        self.controls.insert(
            id.to_string(),
            Control {
                id: id.to_string(),
                title: title.to_string(),
                clause_ref,
                applicable,
                status: ControlStatus::default(),
                evidence: Vec::new(),
                r#ref: None,
                resolved: None,
                implementations: Vec::new(),
            },
        );
        Ok(())
    }

    pub fn set_control_status(
        &mut self,
        id: &str,
        status: ControlStatus,
    ) -> Result<(), DomainError> {
        let c = self
            .controls
            .get_mut(id)
            .ok_or_else(|| DomainError::nf("control", id, ADD_CONTROL_FIX))?;
        // Honesty rule (mirror of ingest_check, SC-5): a reference-backed control
        // derives its status from its source on read — it can NEVER be hand-set to
        // forge a green while the source is failing.
        if c.is_ref_backed() {
            return Err(DomainError::RefBacked {
                kind: "control",
                id: id.to_string(),
                fix: "its status is derived from its source — fix the source, then re-resolve with: muster control resolve <id>".to_string(),
            });
        }
        c.status = status;
        Ok(())
    }

    pub fn attach_control_evidence(
        &mut self,
        id: &str,
        evidence: Evidence,
    ) -> Result<(), DomainError> {
        let c = self
            .controls
            .get_mut(id)
            .ok_or_else(|| DomainError::nf("control", id, ADD_CONTROL_FIX))?;
        c.evidence.push(evidence);
        Ok(())
    }

    // ── v1 glue: reference-backed control / check mutators ─────────────────────

    /// Point a control at an authoritative source (#7). Title/status become
    /// derived on read.
    pub fn set_control_ref(&mut self, id: &str, r: Ref) -> Result<(), DomainError> {
        let c = self
            .controls
            .get_mut(id)
            .ok_or_else(|| DomainError::nf("control", id, ADD_CONTROL_FIX))?;
        c.r#ref = Some(r);
        Ok(())
    }

    /// Cache the last resolution of a control's ref (the cli computes it; domain
    /// stays I/O- and clock-free, #8).
    pub fn set_control_resolution(
        &mut self,
        id: &str,
        resolution: Resolution,
    ) -> Result<(), DomainError> {
        let c = self
            .controls
            .get_mut(id)
            .ok_or_else(|| DomainError::nf("control", id, ADD_CONTROL_FIX))?;
        c.resolved = Some(resolution);
        Ok(())
    }

    /// Append an implementation to a control (N:M, P1). Each implementation
    /// derives its own status from its own ref.
    pub fn add_implementation(
        &mut self,
        cid: &str,
        impl_id: &str,
        r: Ref,
    ) -> Result<(), DomainError> {
        validate_slug(impl_id)?;
        let c = self
            .controls
            .get_mut(cid)
            .ok_or_else(|| DomainError::nf("control", cid, ADD_CONTROL_FIX))?;
        if c.implementations.iter().any(|i| i.id == impl_id) {
            return Err(DomainError::DuplicateId {
                kind: "implementation",
                id: impl_id.to_string(),
            });
        }
        c.implementations.push(Implementation {
            id: impl_id.to_string(),
            r#ref: r,
            resolved: None,
        });
        Ok(())
    }

    /// Cache the last resolution of one implementation's ref.
    pub fn set_implementation_resolution(
        &mut self,
        cid: &str,
        impl_id: &str,
        resolution: Resolution,
    ) -> Result<(), DomainError> {
        let c = self
            .controls
            .get_mut(cid)
            .ok_or_else(|| DomainError::nf("control", cid, ADD_CONTROL_FIX))?;
        let im = c
            .implementations
            .iter_mut()
            .find(|i| i.id == impl_id)
            .ok_or_else(|| {
                DomainError::nf(
                    "implementation",
                    impl_id,
                    "list implementations with: muster control show <id>",
                )
            })?;
        im.resolved = Some(resolution);
        Ok(())
    }

    /// Point a check at an authoritative source (#7). `last_result` becomes
    /// derived on read; the honesty rule then forbids hand-setting it.
    pub fn set_check_ref(&mut self, pid: &str, cid: &str, r: Ref) -> Result<(), DomainError> {
        let check = self.check_mut(pid, cid)?;
        check.r#ref = Some(r);
        Ok(())
    }

    /// Cache the last resolution of a check's ref.
    pub fn set_check_resolution(
        &mut self,
        pid: &str,
        cid: &str,
        resolution: Resolution,
    ) -> Result<(), DomainError> {
        let check = self.check_mut(pid, cid)?;
        check.resolved = Some(resolution);
        Ok(())
    }

    fn check_mut(&mut self, pid: &str, cid: &str) -> Result<&mut Check, DomainError> {
        let p = self
            .processes
            .get_mut(pid)
            .ok_or_else(|| DomainError::nf("process", pid, ADD_PROCESS_FIX))?;
        p.checks.iter_mut().find(|c| c.id == cid).ok_or_else(|| {
            DomainError::nf("check", cid, "list checks with: muster process show <id>")
        })
    }

    // ── incident mutators ────────────────────────────────────────────────────

    pub fn report_incident(
        &mut self,
        id: &str,
        title: &str,
        severity: Severity,
        process_ref: Option<String>,
    ) -> Result<(), DomainError> {
        validate_slug(id)?;
        if self.incidents.contains_key(id) {
            return Err(DomainError::DuplicateId {
                kind: "incident",
                id: id.to_string(),
            });
        }
        if let Some(r) = &process_ref {
            self.require_process(r)?;
        }
        self.incidents.insert(
            id.to_string(),
            Incident {
                id: id.to_string(),
                title: title.to_string(),
                severity,
                status: IncidentStatus::default(),
                process_ref,
                log: Vec::new(),
            },
        );
        Ok(())
    }

    pub fn log_incident(&mut self, id: &str, note: &str, ts: &str) -> Result<(), DomainError> {
        let i = self
            .incidents
            .get_mut(id)
            .ok_or_else(|| DomainError::nf("incident", id, REPORT_INCIDENT_FIX))?;
        i.log.push(LogEntry {
            ts: ts.to_string(),
            note: note.to_string(),
        });
        Ok(())
    }

    pub fn close_incident(&mut self, id: &str) -> Result<(), DomainError> {
        let i = self
            .incidents
            .get_mut(id)
            .ok_or_else(|| DomainError::nf("incident", id, REPORT_INCIDENT_FIX))?;
        i.status = IncidentStatus::Closed;
        Ok(())
    }

    // ── nonconformity mutators ───────────────────────────────────────────────

    #[allow(clippy::too_many_arguments)]
    pub fn raise_nonconformity(
        &mut self,
        id: &str,
        description: &str,
        source: NonconformitySource,
        from_incident: Option<String>,
        process_ref: Option<String>,
        control_ref: Option<String>,
    ) -> Result<(), DomainError> {
        validate_slug(id)?;
        if self.nonconformities.contains_key(id) {
            return Err(DomainError::DuplicateId {
                kind: "nonconformity",
                id: id.to_string(),
            });
        }
        // --from-incident copies the incident's process_ref and forces source=incident.
        let (source, process_ref) = if let Some(iid) = &from_incident {
            let inc = self
                .incidents
                .get(iid)
                .ok_or_else(|| DomainError::mref("incident", iid, REPORT_INCIDENT_FIX))?;
            (NonconformitySource::Incident, inc.process_ref.clone())
        } else {
            if let Some(r) = &process_ref {
                self.require_process(r)?;
            }
            (source, process_ref)
        };
        if let Some(r) = &control_ref {
            self.require_control(r)?;
        }
        self.nonconformities.insert(
            id.to_string(),
            Nonconformity {
                id: id.to_string(),
                source,
                process_ref,
                control_ref,
                description: description.to_string(),
                corrective_action: None,
                status: NonconformityStatus::default(),
            },
        );
        Ok(())
    }

    pub fn resolve_nonconformity(
        &mut self,
        id: &str,
        corrective_action: Option<String>,
    ) -> Result<(), DomainError> {
        let nc = self
            .nonconformities
            .get_mut(id)
            .ok_or_else(|| DomainError::nf("nonconformity", id, RAISE_NC_FIX))?;
        nc.status = NonconformityStatus::Closed;
        if corrective_action.is_some() {
            nc.corrective_action = corrective_action;
        }
        Ok(())
    }

    // ── graph: tree expansion + cycle detection ──────────────────────────────

    /// Depth-first expansion of `step.process_ref`, cycle-safe: a back-edge to a
    /// process already on the current path is emitted as a `cycle` marker and
    /// recursion stops — it never loops forever (SPEC: cycles are reported, not
    /// hung on).
    pub fn show_tree(&self, id: &str) -> Result<TreeView, DomainError> {
        self.process(id)?; // existence (honest not-found)
        let mut path = BTreeSet::new();
        Ok(self.build_tree(id, &mut path))
    }

    fn build_tree(&self, id: &str, path: &mut BTreeSet<String>) -> TreeView {
        path.insert(id.to_string());
        let p = self.processes.get(id);
        let (name, status, steps_src) = match p {
            Some(p) => (p.name.clone(), Some(p.status), p.steps.clone()),
            None => (String::new(), None, Vec::new()),
        };
        let mut steps = Vec::new();
        for s in steps_src {
            let sub = match &s.process_ref {
                Some(r) if path.contains(r) => Some(SubNode::Cycle {
                    process_ref: r.clone(),
                }),
                Some(r) if self.processes.contains_key(r) => {
                    Some(SubNode::Process(Box::new(self.build_tree(r, path))))
                }
                Some(r) => Some(SubNode::Missing {
                    process_ref: r.clone(),
                }),
                None => None,
            };
            steps.push(TreeStep {
                n: s.n,
                description: s.description,
                process_ref: s.process_ref,
                sub,
            });
        }
        path.remove(id);
        TreeView {
            id: id.to_string(),
            name,
            status,
            steps,
        }
    }

    /// All directed cycles in the process graph (nodes = processes, edges =
    /// `process → step.process_ref`). Colored DFS; a grey re-visit is a
    /// back-edge → record the cycle. Always terminates.
    pub fn detect_cycles(&self) -> Vec<Vec<String>> {
        #[derive(Clone, Copy, PartialEq)]
        enum Color {
            White,
            Grey,
            Black,
        }
        let mut color: BTreeMap<&str, Color> = self
            .processes
            .keys()
            .map(|k| (k.as_str(), Color::White))
            .collect();
        let mut cycles: Vec<Vec<String>> = Vec::new();

        // Iterative DFS with an explicit stack of (node, child-index) and a path.
        for start in self.processes.keys() {
            if color[start.as_str()] != Color::White {
                continue;
            }
            let mut stack: Vec<(&str, usize)> = vec![(start.as_str(), 0)];
            let mut path: Vec<&str> = vec![start.as_str()];
            *color.get_mut(start.as_str()).unwrap() = Color::Grey;

            while let Some(&(node, idx)) = stack.last() {
                let succ = self.successors(node);
                if idx < succ.len() {
                    stack.last_mut().unwrap().1 += 1;
                    let next = succ[idx];
                    match color.get(next).copied() {
                        Some(Color::White) => {
                            *color.get_mut(next).unwrap() = Color::Grey;
                            stack.push((next, 0));
                            path.push(next);
                        }
                        Some(Color::Grey) => {
                            // Back-edge: record the cycle from `next` to the top of path.
                            if let Some(pos) = path.iter().position(|n| *n == next) {
                                cycles.push(path[pos..].iter().map(|s| s.to_string()).collect());
                            }
                        }
                        _ => {} // black or missing target — no cycle through it
                    }
                } else {
                    *color.get_mut(node).unwrap() = Color::Black;
                    stack.pop();
                    path.pop();
                }
            }
        }
        cycles
    }

    /// Distinct sub-process ids referenced by a process's steps (graph successors).
    fn successors(&self, id: &str) -> Vec<&str> {
        let mut seen = BTreeSet::new();
        let mut out = Vec::new();
        if let Some(p) = self.processes.get(id) {
            for s in &p.steps {
                if let Some(r) = &s.process_ref
                    && self.processes.contains_key(r)
                    && seen.insert(r.as_str())
                {
                    out.push(r.as_str());
                }
            }
        }
        out
    }

    /// All processes reachable from `start` (inclusive) via `process_ref` edges,
    /// cycle-safe. Used to scope `readiness --process <id>`.
    pub fn reachable(&self, start: &str) -> BTreeSet<String> {
        let mut seen = BTreeSet::new();
        let mut stack = vec![start.to_string()];
        while let Some(n) = stack.pop() {
            if !self.processes.contains_key(&n) || !seen.insert(n.clone()) {
                continue;
            }
            for s in self.successors(&n) {
                stack.push(s.to_string());
            }
        }
        seen
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tree view (graph rendering — distinct from the flat `show`)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TreeView {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<ProcessStatus>,
    pub steps: Vec<TreeStep>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TreeStep {
    pub n: i64,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub: Option<SubNode>,
}

/// The expansion of a step's `process_ref`: a nested process, a cycle marker, or
/// a dangling reference. Honest about each (#1).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SubNode {
    Process(Box<TreeView>),
    Cycle { process_ref: String },
    Missing { process_ref: String },
}

impl std::fmt::Display for TreeView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.render(f, 0)
    }
}

impl TreeView {
    fn render(&self, f: &mut std::fmt::Formatter<'_>, depth: usize) -> std::fmt::Result {
        let pad = "  ".repeat(depth);
        let status = self.status.map(|s| format!(" [{s}]")).unwrap_or_default();
        writeln!(f, "{pad}{} — {}{}", self.id, self.name, status)?;
        for s in &self.steps {
            let spad = "  ".repeat(depth + 1);
            writeln!(f, "{spad}{}. {}", s.n, s.description)?;
            match &s.sub {
                Some(SubNode::Process(t)) => t.render(f, depth + 2)?,
                Some(SubNode::Cycle { process_ref }) => {
                    let cpad = "  ".repeat(depth + 2);
                    writeln!(f, "{cpad}-> {process_ref} (cycle — not expanded)")?;
                }
                Some(SubNode::Missing { process_ref }) => {
                    let cpad = "  ".repeat(depth + 2);
                    writeln!(f, "{cpad}-> {process_ref} (missing process)")?;
                }
                None => {}
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store_with(ids: &[&str]) -> Store {
        let mut s = Store::default();
        for id in ids {
            s.add_process(id, id, None, None).unwrap();
        }
        s
    }

    #[test]
    fn add_process_defaults_to_proposed() {
        let s = store_with(&["p1"]);
        assert_eq!(s.process("p1").unwrap().status, ProcessStatus::Proposed);
    }

    #[test]
    fn add_process_rejects_duplicate_and_bad_slug() {
        let mut s = store_with(&["p1"]);
        assert!(matches!(
            s.add_process("p1", "x", None, None),
            Err(DomainError::DuplicateId { .. })
        ));
        assert!(matches!(
            s.add_process("Bad Id", "x", None, None),
            Err(DomainError::InvalidSlug(_))
        ));
    }

    #[test]
    fn step_with_missing_process_ref_is_rejected() {
        let mut s = store_with(&["p1"]);
        let err = s
            .add_step("p1", "do", None, vec![], Some("ghost".into()))
            .unwrap_err();
        assert!(matches!(err, DomainError::MissingReference { .. }));
    }

    #[test]
    fn link_control_requires_both_to_exist() {
        let mut s = store_with(&["p1"]);
        assert!(s.link_control("p1", "c1").is_err());
        s.add_control("c1", "C", None, true).unwrap();
        s.link_control("p1", "c1").unwrap();
        assert_eq!(s.process("p1").unwrap().controls, vec!["c1".to_string()]);
        // idempotent
        s.link_control("p1", "c1").unwrap();
        assert_eq!(s.process("p1").unwrap().controls.len(), 1);
    }

    #[test]
    fn add_check_generates_ids_and_ingest_sets_result() {
        let mut s = store_with(&["p1"]);
        let cid = s.add_check("p1", "runbook", Enforcement::Ci).unwrap();
        assert_eq!(cid, "check-1");
        assert_eq!(
            s.process("p1").unwrap().checks[0].last_result,
            CheckResult::Unknown
        );
        s.ingest_check("p1", &cid, CheckResult::Pass, "2026-01-01T00:00:00Z", None)
            .unwrap();
        let c = &s.process("p1").unwrap().checks[0];
        assert_eq!(c.last_result, CheckResult::Pass);
        assert_eq!(c.last_run_ts.as_deref(), Some("2026-01-01T00:00:00Z"));
    }

    #[test]
    fn raise_from_incident_copies_process_ref_and_source() {
        let mut s = store_with(&["p1"]);
        s.report_incident("inc-1", "Outage", Severity::High, Some("p1".into()))
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
        let nc = s.nonconformity("nc-1").unwrap();
        assert_eq!(nc.source, NonconformitySource::Incident);
        assert_eq!(nc.process_ref.as_deref(), Some("p1"));
    }

    #[test]
    fn revise_appends_and_validates_because() {
        let mut s = store_with(&["p1"]);
        s.report_incident("inc-1", "O", Severity::Medium, None)
            .unwrap();
        s.revise(
            "p1",
            "tightened",
            Some("inc-1".into()),
            "2026-01-01T00:00:00Z",
        )
        .unwrap();
        assert_eq!(s.process("p1").unwrap().revisions.len(), 1);
        assert_eq!(
            s.process("p1").unwrap().revisions[0].because.as_deref(),
            Some("inc-1")
        );
        // unknown cause rejected
        assert!(s.revise("p1", "x", Some("ghost".into()), "t").is_err());
    }

    #[test]
    fn ingest_check_rejects_ref_backed_check() {
        let mut s = store_with(&["p1"]);
        let cid = s.add_check("p1", "d", Enforcement::Ci).unwrap();
        s.set_check_ref(
            "p1",
            &cid,
            Ref::FileAnchor {
                path: "x.toml".into(),
                anchor: "a.status".into(),
            },
        )
        .unwrap();
        let err = s
            .ingest_check("p1", &cid, CheckResult::Pass, "t", None)
            .unwrap_err();
        assert!(matches!(err, DomainError::RefBacked { .. }), "{err:?}");
    }

    #[test]
    fn set_control_status_rejects_ref_backed_control() {
        // Honesty rule (mirror of ingest_check): a ref-backed control's status is
        // DERIVED from its source on read — it can NEVER be hand-set to forge a
        // green while the source is failing.
        let mut s = Store::default();
        s.add_control("c1", "C", None, true).unwrap();
        s.set_control_ref(
            "c1",
            Ref::FileAnchor {
                path: "x.toml".into(),
                anchor: "a.status".into(),
            },
        )
        .unwrap();
        let err = s
            .set_control_status("c1", ControlStatus::Implemented)
            .unwrap_err();
        assert!(matches!(err, DomainError::RefBacked { .. }), "{err:?}");
        // And the forged status was NOT persisted — the stored status is unchanged.
        assert_eq!(s.control("c1").unwrap().status, ControlStatus::NotStarted);
    }

    #[test]
    fn add_implementation_validates_and_dedups() {
        let mut s = store_with(&["p1"]);
        s.add_control("c1", "C", None, true).unwrap();
        s.add_implementation("c1", "rust", Ref::Note { text: "x".into() })
            .unwrap();
        assert!(matches!(
            s.add_implementation("c1", "rust", Ref::Note { text: "y".into() }),
            Err(DomainError::DuplicateId { .. })
        ));
        assert!(matches!(
            s.add_implementation("c1", "Bad Id", Ref::Note { text: "z".into() }),
            Err(DomainError::InvalidSlug(_))
        ));
        assert_eq!(s.control("c1").unwrap().implementations.len(), 1);
    }

    #[test]
    fn detect_cycles_finds_two_node_cycle_and_terminates() {
        let mut s = store_with(&["a", "b"]);
        s.add_step("a", "to b", None, vec![], Some("b".into()))
            .unwrap();
        s.add_step("b", "to a", None, vec![], Some("a".into()))
            .unwrap();
        let cycles = s.detect_cycles();
        assert!(!cycles.is_empty(), "expected a cycle");
        assert!(cycles[0].contains(&"a".to_string()) && cycles[0].contains(&"b".to_string()));
    }

    #[test]
    fn detect_cycles_finds_three_node_cycle() {
        let mut s = store_with(&["a", "b", "c"]);
        s.add_step("a", "", None, vec![], Some("b".into())).unwrap();
        s.add_step("b", "", None, vec![], Some("c".into())).unwrap();
        s.add_step("c", "", None, vec![], Some("a".into())).unwrap();
        assert_eq!(s.detect_cycles().len(), 1);
    }

    #[test]
    fn acyclic_graph_has_no_cycles() {
        let mut s = store_with(&["a", "b"]);
        s.add_step("a", "", None, vec![], Some("b".into())).unwrap();
        assert!(s.detect_cycles().is_empty());
    }

    #[test]
    fn show_tree_on_cycle_terminates_with_marker() {
        let mut s = store_with(&["a", "b"]);
        s.add_step("a", "to b", None, vec![], Some("b".into()))
            .unwrap();
        s.add_step("b", "to a", None, vec![], Some("a".into()))
            .unwrap();
        let tree = s.show_tree("a").unwrap();
        // a -> b -> (cycle back to a)
        let b_node = match &tree.steps[0].sub {
            Some(SubNode::Process(t)) => t,
            other => panic!("expected expanded b, got {other:?}"),
        };
        assert!(matches!(b_node.steps[0].sub, Some(SubNode::Cycle { .. })));
    }

    #[test]
    fn reachable_is_cycle_safe() {
        let mut s = store_with(&["a", "b"]);
        s.add_step("a", "", None, vec![], Some("b".into())).unwrap();
        s.add_step("b", "", None, vec![], Some("a".into())).unwrap();
        let r = s.reachable("a");
        assert_eq!(r.len(), 2);
    }
}
