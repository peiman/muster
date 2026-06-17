//! muster domain model — the process spine and its satellites.
//!
//! Pure data + validation. No I/O, no clap, no fs (Manifesto #8 Separation of
//! Concerns; enforced by `crates/domain/Cargo.toml`). Every entity is
//! `Serialize + Deserialize` (the JSON-on-disk / JSON-surface SSOT, #7) and
//! implements `Display` (the human surface) so text and JSON tell the *same*
//! story from one source.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

// ─────────────────────────────────────────────────────────────────────────────
// Errors (Manifesto #3 honest signals — every error names the offender and the
// corrective command).
// ─────────────────────────────────────────────────────────────────────────────

/// A typed domain validation error. `Display` names what is wrong *and the fix*.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum DomainError {
    #[error(
        "invalid id '{0}' — ids must be a slug matching ^[a-z][a-z0-9-]*$ \
         (lowercase, start with a letter, hyphen-separated), e.g. 'incident-mgmt'"
    )]
    InvalidSlug(String),

    #[error("{kind} '{id}' already exists — choose a different id or update the existing one")]
    DuplicateId { kind: &'static str, id: String },

    #[error("{kind} '{id}' not found — {fix}")]
    NotFound {
        kind: &'static str,
        id: String,
        fix: String,
    },

    #[error("{kind} '{id}' not found — {fix}")]
    MissingReference {
        kind: &'static str,
        id: String,
        fix: String,
    },

    #[error("a conformance result must be exactly one of --pass or --fail (got {0})")]
    AmbiguousResult(&'static str),
}

impl DomainError {
    pub(crate) fn nf(kind: &'static str, id: impl Into<String>, fix: impl Into<String>) -> Self {
        DomainError::NotFound {
            kind,
            id: id.into(),
            fix: fix.into(),
        }
    }
    pub(crate) fn mref(kind: &'static str, id: impl Into<String>, fix: impl Into<String>) -> Self {
        DomainError::MissingReference {
            kind,
            id: id.into(),
            fix: fix.into(),
        }
    }
}

/// Validate a slug: `^[a-z][a-z0-9-]*$`. Pure (no `regex` dependency — Manifesto
/// #4 minimalism; #9 the check is the enforcement).
pub fn validate_slug(id: &str) -> Result<(), DomainError> {
    let mut chars = id.chars();
    let ok = match chars.next() {
        Some(c) if c.is_ascii_lowercase() => {
            chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        }
        _ => false,
    };
    if ok {
        Ok(())
    } else {
        Err(DomainError::InvalidSlug(id.to_string()))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Enum helper: parse-from-CLI + render. clap stays in the cli crate (#8), so the
// enums expose `FromStr` (clap consumes it) + `Display` (snake_case, matching
// serde) rather than deriving `clap::ValueEnum` here.
// ─────────────────────────────────────────────────────────────────────────────

macro_rules! str_enum {
    (
        $(#[$meta:meta])*
        $name:ident { $( $variant:ident => $s:literal ),+ $(,)? } default = $default:ident
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $( $variant ),+ }

        impl $name {
            /// The allowed string values, in declaration order (for error text + help).
            pub const VALUES: &'static [&'static str] = &[ $( $s ),+ ];
            pub fn as_str(&self) -> &'static str {
                match self { $( $name::$variant => $s ),+ }
            }
        }

        impl Default for $name {
            fn default() -> Self { $name::$default }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl FromStr for $name {
            type Err = String;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $( $s => Ok($name::$variant), )+
                    other => Err(format!(
                        "invalid {} '{}' — expected one of: {}",
                        stringify!($name), other, Self::VALUES.join(", ")
                    )),
                }
            }
        }
    };
}

str_enum! {
    /// The hypothesis lifecycle of a process (#10): a process is `proposed`
    /// (unproven) → `active` → `under_review` (reality diverging) → `retired`.
    ProcessStatus {
        Proposed => "proposed",
        Active => "active",
        UnderReview => "under_review",
        Retired => "retired",
    } default = Proposed
}

str_enum! {
    /// Implementation state of a control.
    ControlStatus {
        NotStarted => "not_started",
        InProgress => "in_progress",
        Implemented => "implemented",
    } default = NotStarted
}

str_enum! {
    Severity {
        Low => "low",
        Medium => "medium",
        High => "high",
        Critical => "critical",
    } default = Medium
}

str_enum! {
    IncidentStatus {
        Open => "open",
        Mitigating => "mitigating",
        Closed => "closed",
    } default = Open
}

str_enum! {
    /// Where a nonconformity finding came from.
    NonconformitySource {
        Incident => "incident",
        Audit => "audit",
        Manual => "manual",
    } default = Manual
}

str_enum! {
    NonconformityStatus {
        Open => "open",
        InProgress => "in_progress",
        Closed => "closed",
    } default = Open
}

str_enum! {
    /// The #9 enforcement ladder, strongest → weakest. `rank()` encodes the
    /// ordering "compile_time > lint > script > ci > honor".
    Enforcement {
        CompileTime => "compile_time",
        Lint => "lint",
        Script => "script",
        Ci => "ci",
        Honor => "honor",
    } default = Honor
}

impl Enforcement {
    /// Strength rank: higher == stronger enforcement (#9 ladder).
    pub fn rank(&self) -> u8 {
        match self {
            Enforcement::CompileTime => 5,
            Enforcement::Lint => 4,
            Enforcement::Script => 3,
            Enforcement::Ci => 2,
            Enforcement::Honor => 1,
        }
    }
}

str_enum! {
    CheckResult {
        Pass => "pass",
        Fail => "fail",
        Unknown => "unknown",
    } default = Unknown
}

str_enum! {
    EvidenceKind {
        File => "file",
        Url => "url",
        Note => "note",
    } default = Note
}

// ─────────────────────────────────────────────────────────────────────────────
// Evidence
// ─────────────────────────────────────────────────────────────────────────────

/// A reference attached to a process / control / nonconformity / check.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Evidence {
    pub kind: EvidenceKind,
    pub value: String,
}

impl fmt::Display for Evidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.kind, self.value)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Process — the spine
// ─────────────────────────────────────────────────────────────────────────────

/// An ordered activity inside a process; the recursion point for the graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Step {
    pub n: i64,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(default)]
    pub controls: Vec<String>,
    /// Id of a sub-process this step delegates to → processes compose recursively.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process_ref: Option<String>,
}

/// A conformance signal — the #9 Automated Enforcement seam (the CI plugin's
/// future entry point, #5 platform not feature).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Check {
    pub id: String,
    pub description: String,
    pub enforcement: Enforcement,
    pub last_result: CheckResult,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_run_ts: Option<String>,
    #[serde(default)]
    pub evidence: Vec<Evidence>,
}

/// The feedback cycle made auditable (#10): append-only record of why a process
/// hypothesis changed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Revision {
    pub ts: String,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub because: Option<String>,
}

/// A process: a node in a directed graph; a *hypothesis* about how work is done.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Process {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub purpose: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    pub status: ProcessStatus,
    #[serde(default)]
    pub inputs: Vec<String>,
    #[serde(default)]
    pub outputs: Vec<String>,
    #[serde(default)]
    pub steps: Vec<Step>,
    #[serde(default)]
    pub controls: Vec<String>,
    #[serde(default)]
    pub risks: Vec<String>,
    #[serde(default)]
    pub metrics: Vec<String>,
    #[serde(default)]
    pub checks: Vec<Check>,
    #[serde(default)]
    pub revisions: Vec<Revision>,
    #[serde(default)]
    pub evidence: Vec<Evidence>,
}

impl Process {
    pub fn new(id: String, name: String) -> Self {
        Process {
            id,
            name,
            purpose: None,
            owner: None,
            status: ProcessStatus::default(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            steps: Vec::new(),
            controls: Vec::new(),
            risks: Vec::new(),
            metrics: Vec::new(),
            checks: Vec::new(),
            revisions: Vec::new(),
            evidence: Vec::new(),
        }
    }

    /// The strongest enforcement among this process's checks (#9 ladder), or
    /// `None` if it has no checks.
    pub fn strongest_enforcement(&self) -> Option<Enforcement> {
        self.checks
            .iter()
            .map(|c| c.enforcement)
            .max_by_key(|e| e.rank())
    }
}

impl fmt::Display for Process {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "process {} — {}", self.id, self.name)?;
        writeln!(f, "  status: {}", self.status)?;
        if let Some(p) = &self.purpose {
            writeln!(f, "  purpose: {p}")?;
        }
        if let Some(o) = &self.owner {
            writeln!(f, "  owner: {o}")?;
        }
        write_list(f, "inputs", &self.inputs)?;
        write_list(f, "outputs", &self.outputs)?;
        if !self.steps.is_empty() {
            writeln!(f, "  steps:")?;
            for s in &self.steps {
                let r = s
                    .process_ref
                    .as_ref()
                    .map(|r| format!(" -> {r}"))
                    .unwrap_or_default();
                writeln!(f, "    {}. {}{}", s.n, s.description, r)?;
            }
        }
        write_list(f, "controls", &self.controls)?;
        write_list(f, "risks", &self.risks)?;
        write_list(f, "metrics", &self.metrics)?;
        if !self.checks.is_empty() {
            writeln!(f, "  checks:")?;
            for c in &self.checks {
                writeln!(
                    f,
                    "    {} [{}] {} = {}",
                    c.id, c.enforcement, c.description, c.last_result
                )?;
            }
        }
        if !self.revisions.is_empty() {
            writeln!(f, "  revisions:")?;
            for r in &self.revisions {
                let because = r
                    .because
                    .as_ref()
                    .map(|b| format!(" (because {b})"))
                    .unwrap_or_default();
                writeln!(f, "    {} — {}{}", r.ts, r.summary, because)?;
            }
        }
        if !self.evidence.is_empty() {
            writeln!(f, "  evidence:")?;
            for e in &self.evidence {
                writeln!(f, "    {e}")?;
            }
        }
        Ok(())
    }
}

fn write_list(f: &mut fmt::Formatter<'_>, label: &str, items: &[String]) -> fmt::Result {
    if !items.is_empty() {
        writeln!(f, "  {label}: {}", items.join(", "))?;
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Control
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Control {
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clause_ref: Option<String>,
    pub applicable: bool,
    pub status: ControlStatus,
    #[serde(default)]
    pub evidence: Vec<Evidence>,
}

impl fmt::Display for Control {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "control {} — {}", self.id, self.title)?;
        writeln!(f, "  status: {}", self.status)?;
        writeln!(f, "  applicable: {}", self.applicable)?;
        if let Some(c) = &self.clause_ref {
            writeln!(f, "  clause_ref: {c}")?;
        }
        if !self.evidence.is_empty() {
            writeln!(f, "  evidence:")?;
            for e in &self.evidence {
                writeln!(f, "    {e}")?;
            }
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Incident
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LogEntry {
    pub ts: String,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Incident {
    pub id: String,
    pub title: String,
    pub severity: Severity,
    pub status: IncidentStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process_ref: Option<String>,
    #[serde(default)]
    pub log: Vec<LogEntry>,
}

impl fmt::Display for Incident {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "incident {} — {}", self.id, self.title)?;
        writeln!(f, "  severity: {}", self.severity)?;
        writeln!(f, "  status: {}", self.status)?;
        if let Some(p) = &self.process_ref {
            writeln!(f, "  process: {p}")?;
        }
        if !self.log.is_empty() {
            writeln!(f, "  log:")?;
            for e in &self.log {
                writeln!(f, "    {} — {}", e.ts, e.note)?;
            }
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Nonconformity
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Nonconformity {
    pub id: String,
    pub source: NonconformitySource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control_ref: Option<String>,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corrective_action: Option<String>,
    pub status: NonconformityStatus,
}

impl fmt::Display for Nonconformity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "nonconformity {} — {}", self.id, self.description)?;
        writeln!(f, "  source: {}", self.source)?;
        writeln!(f, "  status: {}", self.status)?;
        if let Some(p) = &self.process_ref {
            writeln!(f, "  process: {p}")?;
        }
        if let Some(c) = &self.control_ref {
            writeln!(f, "  control: {c}")?;
        }
        if let Some(a) = &self.corrective_action {
            writeln!(f, "  corrective_action: {a}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_accepts_valid() {
        for s in ["a", "incident-mgmt", "a5-24", "x1"] {
            assert!(validate_slug(s).is_ok(), "{s} should be valid");
        }
    }

    #[test]
    fn slug_rejects_invalid() {
        for s in ["", "1abc", "Abc", "has space", "-leading", "under_score", "trailing-OK!"] {
            assert!(validate_slug(s).is_err(), "{s} should be invalid");
        }
    }

    #[test]
    fn enum_from_str_roundtrips() {
        assert_eq!("ci".parse::<Enforcement>().unwrap(), Enforcement::Ci);
        assert_eq!(Enforcement::Ci.to_string(), "ci");
        assert_eq!("under_review".parse::<ProcessStatus>().unwrap(), ProcessStatus::UnderReview);
    }

    #[test]
    fn enum_from_str_rejects_garbage_naming_allowed_values() {
        let err = "nope".parse::<Enforcement>().unwrap_err();
        assert!(err.contains("compile_time"), "error must name allowed values: {err}");
        assert!(err.contains("honor"));
    }

    #[test]
    fn enforcement_ladder_ranks_compile_time_strongest() {
        assert!(Enforcement::CompileTime.rank() > Enforcement::Lint.rank());
        assert!(Enforcement::Lint.rank() > Enforcement::Script.rank());
        assert!(Enforcement::Script.rank() > Enforcement::Ci.rank());
        assert!(Enforcement::Ci.rank() > Enforcement::Honor.rank());
    }

    #[test]
    fn process_strongest_enforcement_picks_max() {
        let mut p = Process::new("p".into(), "P".into());
        assert_eq!(p.strongest_enforcement(), None);
        p.checks.push(Check {
            id: "c1".into(),
            description: "d".into(),
            enforcement: Enforcement::Honor,
            last_result: CheckResult::Unknown,
            last_run_ts: None,
            evidence: vec![],
        });
        p.checks.push(Check {
            id: "c2".into(),
            description: "d".into(),
            enforcement: Enforcement::Ci,
            last_result: CheckResult::Unknown,
            last_run_ts: None,
            evidence: vec![],
        });
        assert_eq!(p.strongest_enforcement(), Some(Enforcement::Ci));
    }
}
