//! muster domain model — the process spine and its satellites.
//!
//! Pure data + validation. No I/O, no clap, no fs (Manifesto #8 Separation of
//! Concerns; enforced by `crates/domain/Cargo.toml`). Every entity is
//! `Serialize + Deserialize` (the JSON-on-disk / JSON-surface SSOT, #7) and
//! implements `Display` (the human surface) so text and JSON tell the *same*
//! story from one source.

use crate::reference::{Derived, Outcome, Ref, Resolution};
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

    #[error("{kind} '{id}' is reference-backed — {fix}")]
    RefBacked {
        kind: &'static str,
        id: String,
        fix: String,
    },
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

impl EvidenceKind {
    /// Whether this kind is *verifying* — it points at an inspectable artifact
    /// (`file`/`url`) — versus *honor-level* (`note`: an unverified "I did it"
    /// assertion). The honesty seam (#1): a note alone never proves coverage,
    /// symmetric with how a note *ref* projects to `Asserted` and is never
    /// green-eligible. `readiness` requires at least one verifying evidence
    /// before counting a hand-set control as implemented-with-evidence.
    pub fn is_verifying(&self) -> bool {
        matches!(self, EvidenceKind::File | EvidenceKind::Url)
    }
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

/// `true` when at least one attached evidence is *verifying* (a file/url
/// artifact), not merely an honor-level note. The SSOT predicate `readiness`
/// uses to keep a note-only claim off the READY headline (#1). Empty ⇒ false.
pub fn has_verifying_evidence(evidence: &[Evidence]) -> bool {
    evidence.iter().any(|e| e.kind.is_verifying())
}

/// FORMAT-only URL validation (v1 is NO-NETWORK — this is **never** a
/// reachability / HTTP check). Well-formed ⇔ a case-insensitive `http`/`https`
/// scheme, then `://`, then a non-empty host. Pure std string ops (no `url`
/// crate, no I/O — stays domain-pure, #8). Rejects `""`, whitespace, `notaurl`
/// (no scheme), `ftp://h` (wrong scheme), `http://` (empty host), `://x` (empty
/// scheme). A host that is empty-after-trim (`http://  `) or contains inner
/// whitespace (`https://x /y`) is malformed and rejected.
pub fn is_well_formed_url(value: &str) -> bool {
    let Some((scheme, rest)) = value.split_once("://") else {
        return false;
    };
    let scheme = scheme.to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        return false;
    }
    // Host is everything up to the first '/', '?' or '#'; must be non-empty and
    // whitespace-free (a blank or space-containing host is malformed).
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    !host.trim().is_empty() && !host.chars().any(char::is_whitespace)
}

/// The honest verifying verdict for an evidence list, given a file-existence
/// ORACLE supplied by the caller (the cli passes `Path::is_file`; the domain
/// invokes the closure but does no fs itself, keeping #8). URL format is checked
/// purely. Returns `Verified` on the FIRST evidence whose artifact resolves; else
/// `Empty` (no evidence), `NoteOnly` (only honor-level notes), or `Unresolved`
/// naming the FIRST verifying-kind (file/url) item that did not resolve so the
/// readiness layer can name the offender + the fix (#3 honest signals).
pub fn verify_evidence(
    evidence: &[Evidence],
    file_exists: impl Fn(&str) -> bool,
) -> EvidenceVerdict {
    if evidence.is_empty() {
        return EvidenceVerdict::Empty;
    }
    let mut first_unresolved: Option<EvidenceVerdict> = None;
    let mut saw_verifying_kind = false;
    for e in evidence {
        let resolves = match e.kind {
            EvidenceKind::File => {
                saw_verifying_kind = true;
                file_exists(&e.value)
            }
            EvidenceKind::Url => {
                saw_verifying_kind = true;
                is_well_formed_url(&e.value)
            }
            EvidenceKind::Note => false,
        };
        if resolves {
            return EvidenceVerdict::Verified;
        }
        if e.kind.is_verifying() && first_unresolved.is_none() {
            let reason = match e.kind {
                EvidenceKind::File => "missing or not a regular file (paths resolve relative to the current directory)".to_string(),
                EvidenceKind::Url => "malformed url — needs an http(s):// scheme and a host".to_string(),
                EvidenceKind::Note => unreachable!("note is not a verifying kind"),
            };
            first_unresolved = Some(EvidenceVerdict::Unresolved {
                kind: e.kind,
                value: e.value.clone(),
                reason,
            });
        }
    }
    match first_unresolved {
        Some(v) => v,
        None if saw_verifying_kind => unreachable!("a verifying kind always sets first_unresolved"),
        None => EvidenceVerdict::NoteOnly,
    }
}

/// The honest verifying verdict for a control's / process's evidence list (the
/// honor-VERIFIED upgrade of `has_verifying_evidence`). Built by the cli (which
/// injects file-existence) and threaded into `readiness_with`; controls absent
/// from that index fall back to the kind-only `has_verifying_evidence`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum EvidenceVerdict {
    /// ≥1 verifying artifact resolves (an existing file or a well-formed url).
    Verified,
    /// Has a verifying-kind evidence (file/url) but NONE resolve — names the
    /// first offender + a human reason so readiness can guide the fix.
    Unresolved {
        kind: EvidenceKind,
        value: String,
        reason: String,
    },
    /// Only honor-level note(s) — never proves coverage (b1 invariant).
    NoteOnly,
    /// No evidence at all.
    Empty,
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
    /// v1 glue: a typed pointer to the authoritative source (#7). When present,
    /// `last_result` is DERIVED on read by resolving the ref — never hand-set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#ref: Option<Ref>,
    /// Cache of the last resolution (for display / `command`-kind refs).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved: Option<Resolution>,
}

impl Check {
    /// `true` when this check derives its result from a ref (the honesty rule
    /// applies — it cannot be hand-set, SC-5).
    pub fn is_ref_backed(&self) -> bool {
        self.r#ref.is_some()
    }

    /// The honest result of this check. Ref-backed ⇒ the derived outcome
    /// (`Pass→Pass`, `Fail→Fail`, `Unknown→Unknown`), **ignoring** any stored
    /// `last_result` (closes the honesty hole, SC-5). No ref ⇒ stored
    /// `last_result` (v0 path). The `derived` projection is supplied by the cli
    /// (which owns the clock + resolution).
    pub fn effective_result(&self, derived: Option<&Derived>) -> CheckResult {
        match (&self.r#ref, derived) {
            (Some(_), Some(d)) => match d.outcome() {
                Outcome::Pass => CheckResult::Pass,
                Outcome::Fail => CheckResult::Fail,
                Outcome::Unknown => CheckResult::Unknown,
            },
            // Ref-backed but no resolution available ⇒ honestly unknown.
            (Some(_), None) => CheckResult::Unknown,
            (None, _) => self.last_result,
        }
    }
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

/// One implementation of a control's requirement, with its own derived status
/// (P1 N:M — one requirement satisfied by many implementations, each resolving
/// its own source).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Implementation {
    pub id: String,
    pub r#ref: Ref,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved: Option<Resolution>,
}

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
    /// v1 glue: a typed pointer to the authoritative source (#7). When present,
    /// `title` and `status` are DERIVED on read — the stored `title` is only a
    /// fallback display label, never the authority.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#ref: Option<Ref>,
    /// Cache of the last resolution (for display / `command`-kind refs).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved: Option<Resolution>,
    /// N:M implementations, each deriving its own status (P1).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub implementations: Vec<Implementation>,
}

impl Control {
    /// `true` when this control's own status is derived from a ref (#7).
    pub fn is_ref_backed(&self) -> bool {
        self.r#ref.is_some()
    }

    /// The title to display: the resolved value when the control's ref currently
    /// resolves to a value, else the stored `title` (fallback label).
    pub fn display_title(&self, derived: Option<&Derived>) -> String {
        match (self.is_ref_backed(), derived) {
            (true, Some(Derived::Derived { value, .. }))
            | (true, Some(Derived::Stale { value, .. })) => value.clone(),
            _ => self.title.clone(),
        }
    }

    /// The honest implementation status. When a ref or implementations are
    /// present the status is DERIVED: green-eligible (`Implemented`) only if the
    /// control's own ref (if any) is freshly `Derived` + non-`Fail` AND **every**
    /// implementation projects to `Derived` + `Pass`. Any `Fail`/`Unresolved`/
    /// `Stale` forces a non-`Implemented` status. No ref + no impls ⇒ the stored
    /// hand-set status (asserted, v0 path).
    ///
    /// `own` is the control's own ref projection; `impls` are the per-
    /// implementation projections (same order as `self.implementations`). Both
    /// are supplied by the cli, which owns the clock + resolution.
    pub fn effective_status(&self, own: Option<&Derived>, impls: &[Derived]) -> ControlStatus {
        if !self.is_ref_backed() && self.implementations.is_empty() {
            return self.status;
        }
        // A projection blocks green when its source is Fail/Unresolved/Stale (or a
        // declared ref that produced no resolution at all). A title-only `Derived`
        // (Unknown outcome) neither blocks nor proves.
        let blocks = |d: &Derived| {
            matches!(
                d,
                Derived::Unresolved { .. }
                    | Derived::Stale { .. }
                    | Derived::Derived {
                        outcome: Outcome::Fail,
                        ..
                    }
            )
        };
        let own_blocks = match (self.is_ref_backed(), own) {
            (false, _) => false,
            (true, Some(d)) => blocks(d),
            (true, None) => true, // ref declared but no resolution ⇒ honestly blocked
        };
        if own_blocks || impls.iter().any(blocks) {
            return ControlStatus::InProgress;
        }
        // Nothing blocks. Green only when an actual Pass exists somewhere.
        let any_pass = own.is_some_and(Derived::is_green_eligible)
            || impls.iter().any(Derived::is_green_eligible);
        if any_pass {
            ControlStatus::Implemented
        } else {
            ControlStatus::InProgress
        }
    }

    /// The honest display projection of the control as a whole, for `readiness`
    /// and rendering. `Asserted` when no ref/impls; otherwise the most honest
    /// (worst) of the own ref and the implementations: any `Unresolved` ⇒
    /// `Unresolved`, any `Stale` ⇒ `Stale`, any non-green-eligible derived ⇒ that
    /// projection, else the own projection (or the first impl when there is no
    /// own ref).
    pub fn project(&self, own: Option<Derived>, impls: Vec<Derived>) -> Derived {
        if !self.is_ref_backed() && self.implementations.is_empty() {
            return Derived::Asserted;
        }
        let mut all: Vec<Derived> = Vec::new();
        if let Some(d) = own {
            all.push(d);
        }
        all.extend(impls);
        // Honest worst-case: Unresolved dominates, then Stale, then Fail.
        if let Some(u) = all.iter().find(|d| matches!(d, Derived::Unresolved { .. })) {
            return u.clone();
        }
        if let Some(s) = all.iter().find(|d| matches!(d, Derived::Stale { .. })) {
            return s.clone();
        }
        if let Some(f) = all.iter().find(|d| matches!(d.outcome(), Outcome::Fail)) {
            return f.clone();
        }
        // All green-eligible or title-only → return the first projection (the
        // own ref if any, else the first impl).
        all.into_iter().next().unwrap_or(Derived::Asserted)
    }
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
        if !self.implementations.is_empty() {
            writeln!(f, "  implementations:")?;
            for i in &self.implementations {
                writeln!(f, "    {}", i.id)?;
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
        for s in [
            "",
            "1abc",
            "Abc",
            "has space",
            "-leading",
            "under_score",
            "trailing-OK!",
        ] {
            assert!(validate_slug(s).is_err(), "{s} should be invalid");
        }
    }

    #[test]
    fn enum_from_str_roundtrips() {
        assert_eq!("ci".parse::<Enforcement>().unwrap(), Enforcement::Ci);
        assert_eq!(Enforcement::Ci.to_string(), "ci");
        assert_eq!(
            "under_review".parse::<ProcessStatus>().unwrap(),
            ProcessStatus::UnderReview
        );
    }

    #[test]
    fn enum_from_str_rejects_garbage_naming_allowed_values() {
        let err = "nope".parse::<Enforcement>().unwrap_err();
        assert!(
            err.contains("compile_time"),
            "error must name allowed values: {err}"
        );
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
    fn evidence_kind_verifying_vs_honor_level() {
        // A file/url points at an inspectable artifact (verifying); a note is an
        // honor-level assertion ("I did it, trust me"). Symmetric with how a
        // note *ref* projects to `Asserted` — never alone proves coverage (#1).
        assert!(EvidenceKind::File.is_verifying());
        assert!(EvidenceKind::Url.is_verifying());
        assert!(!EvidenceKind::Note.is_verifying());
    }

    #[test]
    fn has_verifying_evidence_requires_a_non_note_artifact() {
        let note = |v: &str| Evidence {
            kind: EvidenceKind::Note,
            value: v.into(),
        };
        let file = Evidence {
            kind: EvidenceKind::File,
            value: "report.json".into(),
        };
        // empty ⇒ none.
        assert!(!has_verifying_evidence(&[]));
        // note-only ⇒ honor-level, not verifying.
        assert!(!has_verifying_evidence(&[note("did it"), note("really")]));
        // at least one file/url ⇒ verifying (mixed is fine).
        assert!(has_verifying_evidence(&[note("did it"), file]));
    }

    #[test]
    fn url_format_matrix() {
        // FORMAT-only (v1 NO-NETWORK): well-formed ⇔ http/https scheme + "://" +
        // non-empty host. Never a reachability/HTTP check.
        for bad in [
            "",
            "   ",
            "notaurl",
            "ftp://host/x",
            "http://",
            "://x",
            "https://",
            // whitespace-host (FALSE-PASS guard): empty-after-trim or inner space.
            "http://  ",
            "https://x /y",
            "http:// ",
            "https://ho st",
            // empty host BEFORE the query/fragment delimiter is still no host.
            "https://?q",
            "https://#frag",
        ] {
            assert!(
                !is_well_formed_url(bad),
                "{bad:?} must NOT be well-formed (FALSE-PASS guard)"
            );
        }
        for good in [
            "https://x/y",
            "http://example.com",
            "HTTPS://Example.com/Path",
            "https://host:8080/p",
            // host extraction before the first '/', '?', '#' (query/fragment branch).
            "https://host/p?q",
            "https://host#frag",
        ] {
            assert!(is_well_formed_url(good), "{good:?} must be well-formed");
        }
    }

    #[test]
    fn verify_evidence_matrix() {
        let file = |v: &str| Evidence {
            kind: EvidenceKind::File,
            value: v.into(),
        };
        let url = |v: &str| Evidence {
            kind: EvidenceKind::Url,
            value: v.into(),
        };
        let note = |v: &str| Evidence {
            kind: EvidenceKind::Note,
            value: v.into(),
        };
        let absent = |_: &str| false;
        let present = |_: &str| true;

        // empty list ⇒ Empty.
        assert_eq!(verify_evidence(&[], absent), EvidenceVerdict::Empty);
        // note-only ⇒ NoteOnly (b1 invariant preserved).
        assert_eq!(
            verify_evidence(&[note("did it"), note("really")], absent),
            EvidenceVerdict::NoteOnly
        );
        // missing file (THE false-pass guard) ⇒ Unresolved naming the file.
        match verify_evidence(&[file("x.pdf")], absent) {
            EvidenceVerdict::Unresolved { kind, value, .. } => {
                assert_eq!(kind, EvidenceKind::File);
                assert_eq!(value, "x.pdf");
            }
            other => panic!("missing file must be Unresolved, got {other:?}"),
        }
        // directory / not-a-file ⇒ oracle false ⇒ Unresolved.
        assert!(matches!(
            verify_evidence(&[file("somedir")], absent),
            EvidenceVerdict::Unresolved { .. }
        ));
        // empty / whitespace path ⇒ Unresolved, no panic.
        assert!(matches!(
            verify_evidence(&[file("")], absent),
            EvidenceVerdict::Unresolved { .. }
        ));
        assert!(matches!(
            verify_evidence(&[file("   ")], absent),
            EvidenceVerdict::Unresolved { .. }
        ));
        // existing file ⇒ Verified.
        assert_eq!(
            verify_evidence(&[file("x")], present),
            EvidenceVerdict::Verified
        );
        // malformed url ⇒ Unresolved naming the url (oracle irrelevant — pure).
        match verify_evidence(&[url("notaurl")], absent) {
            EvidenceVerdict::Unresolved { kind, value, .. } => {
                assert_eq!(kind, EvidenceKind::Url);
                assert_eq!(value, "notaurl");
            }
            other => panic!("malformed url must be Unresolved, got {other:?}"),
        }
        // well-formed url ⇒ Verified (no network).
        assert_eq!(
            verify_evidence(&[url("https://x/y")], absent),
            EvidenceVerdict::Verified
        );
        // mixed: note + missing file ⇒ Unresolved (first verifying-kind offender).
        match verify_evidence(&[note("n"), file("gone.pdf")], absent) {
            EvidenceVerdict::Unresolved { kind, value, .. } => {
                assert_eq!(kind, EvidenceKind::File);
                assert_eq!(value, "gone.pdf");
            }
            other => panic!("expected Unresolved, got {other:?}"),
        }
        // mixed: missing file + existing file ⇒ Verified (any resolving wins).
        assert_eq!(
            verify_evidence(&[file("gone"), file("here")], |p| p == "here"),
            EvidenceVerdict::Verified
        );
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
            r#ref: None,
            resolved: None,
        });
        p.checks.push(Check {
            id: "c2".into(),
            description: "d".into(),
            enforcement: Enforcement::Ci,
            last_result: CheckResult::Unknown,
            last_run_ts: None,
            evidence: vec![],
            r#ref: None,
            resolved: None,
        });
        assert_eq!(p.strongest_enforcement(), Some(Enforcement::Ci));
    }

    // ── v1 glue: projection + honesty (SC-2 domain / SC-5 / SC-10) ────────────

    use crate::reference::{Derived, Outcome, Ref};

    fn ctrl(id: &str, title: &str) -> Control {
        Control {
            id: id.into(),
            title: title.into(),
            clause_ref: None,
            applicable: true,
            status: ControlStatus::NotStarted,
            evidence: vec![],
            r#ref: None,
            resolved: None,
            implementations: vec![],
        }
    }

    fn derived_pass(value: &str) -> Derived {
        Derived::Derived {
            value: value.into(),
            outcome: Outcome::Pass,
            resolved_ts: "2026-01-01T00:00:00Z".into(),
            source_excerpt: None,
            resolved_age_secs: 0,
            served_from_cache: false,
            source_age_secs: None,
        }
    }
    fn derived_fail(value: &str) -> Derived {
        Derived::Derived {
            value: value.into(),
            outcome: Outcome::Fail,
            resolved_ts: "2026-01-01T00:00:00Z".into(),
            source_excerpt: None,
            resolved_age_secs: 0,
            served_from_cache: false,
            source_age_secs: None,
        }
    }

    #[test]
    fn display_title_prefers_resolved_value_over_stored_fallback() {
        let mut c = ctrl("c1", "placeholder");
        c.r#ref = Some(Ref::FileAnchor {
            path: "x.toml".into(),
            anchor: "a.title".into(),
            expect: None,
        });
        let d = Derived::Derived {
            value: "Alpha".into(),
            outcome: Outcome::Unknown,
            resolved_ts: "t".into(),
            source_excerpt: None,
            resolved_age_secs: 0,
            served_from_cache: false,
            source_age_secs: None,
        };
        assert_eq!(c.display_title(Some(&d)), "Alpha");
        // No resolution available ⇒ fall back to stored title.
        assert_eq!(c.display_title(None), "placeholder");
    }

    #[test]
    fn effective_status_honesty_ref_fail_is_never_implemented() {
        let mut c = ctrl("c1", "t");
        c.status = ControlStatus::Implemented; // operator hand-set green
        c.r#ref = Some(Ref::Command {
            cmd: "just check".into(),
            dir: ".".into(),
        });
        // Source says fail ⇒ derived status must NOT be Implemented.
        let status = c.effective_status(Some(&derived_fail("fail")), &[]);
        assert_eq!(status, ControlStatus::InProgress);
    }

    #[test]
    fn effective_status_no_ref_is_asserted_passthrough() {
        let mut c = ctrl("c1", "t");
        c.status = ControlStatus::Implemented;
        assert_eq!(c.effective_status(None, &[]), ControlStatus::Implemented);
    }

    #[test]
    fn effective_status_nm_one_fail_blocks_green() {
        let mut c = ctrl("c1", "t");
        c.implementations = vec![
            Implementation {
                id: "rust".into(),
                r#ref: Ref::Note { text: "x".into() },
                resolved: None,
            },
            Implementation {
                id: "go".into(),
                r#ref: Ref::Note { text: "y".into() },
                resolved: None,
            },
        ];
        // one met, one unmet ⇒ aggregate not green.
        let status = c.effective_status(None, &[derived_pass("met"), derived_fail("unmet")]);
        assert_eq!(status, ControlStatus::InProgress);
        // both met ⇒ implemented.
        let status = c.effective_status(None, &[derived_pass("met"), derived_pass("met")]);
        assert_eq!(status, ControlStatus::Implemented);
    }

    #[test]
    fn project_surfaces_worst_case() {
        let c = ctrl("c1", "t");
        // asserted when no ref/impls.
        assert_eq!(c.project(None, vec![]), Derived::Asserted);
        // unresolved dominates (control made ref-backed + one impl).
        let mut c2 = ctrl("c2", "t");
        c2.r#ref = Some(Ref::Note { text: "x".into() });
        c2.implementations = vec![Implementation {
            id: "go".into(),
            r#ref: Ref::Note { text: "y".into() },
            resolved: None,
        }];
        let proj = c2.project(
            Some(derived_pass("met")),
            vec![Derived::Unresolved {
                reason: "missing".into(),
            }],
        );
        assert!(matches!(proj, Derived::Unresolved { .. }));
    }

    #[test]
    fn check_effective_result_ignores_stored_when_ref_backed() {
        let mut c = Check {
            id: "check-1".into(),
            description: "d".into(),
            enforcement: Enforcement::Ci,
            last_result: CheckResult::Pass, // operator forged a pass
            last_run_ts: None,
            evidence: vec![],
            r#ref: Some(Ref::FileAnchor {
                path: "x.toml".into(),
                anchor: "a.status".into(),
                expect: None,
            }),
            resolved: None,
        };
        // source says fail ⇒ effective result is fail regardless of stored Pass.
        assert_eq!(
            c.effective_result(Some(&derived_fail("unmet"))),
            CheckResult::Fail
        );
        // no ref ⇒ stored last_result.
        c.r#ref = None;
        assert_eq!(c.effective_result(None), CheckResult::Pass);
    }

    #[test]
    fn no_ref_control_serializes_byte_identical_to_v0() {
        // A v0 control JSON (no ref/resolved/implementations) round-trips with no
        // new keys appearing (SC-2 backward compatibility).
        let v0 =
            r#"{"id":"c1","title":"C1","applicable":true,"status":"not_started","evidence":[]}"#;
        let c: Control = serde_json::from_str(v0).unwrap();
        let out = serde_json::to_string(&c).unwrap();
        assert!(!out.contains("\"ref\""), "ref leaked: {out}");
        assert!(!out.contains("resolved"), "resolved leaked: {out}");
        assert!(!out.contains("implementations"), "impls leaked: {out}");
    }
}
