use clap::{Args, Parser, Subcommand, ValueEnum};
use domain::{
    ControlStatus, Enforcement, EvidenceKind, NonconformitySource, ProcessStatus, Ref, Severity,
};

/// Shared `--ref-*` flags for pointing an entity at an authoritative source (#7).
/// At most one ref kind: file_anchor (`--ref-file` + `--ref-anchor`), command
/// (`--ref-cmd` + `--ref-dir`), or note (`--ref-note`). clap enforces the mutual
/// exclusion + the file/cmd pairing.
#[derive(Args, Debug, Clone)]
pub struct RefFlags {
    /// file_anchor: path to a TOML/JSON source file
    #[arg(long = "ref-file", requires = "ref_anchor", conflicts_with_all = ["ref_cmd", "ref_note", "ref_report"])]
    pub ref_file: Option<String>,
    /// file_anchor: dotted anchor of the scalar to read (e.g. requirements.R1.title)
    #[arg(long = "ref-anchor")]
    pub ref_anchor: Option<String>,
    /// command: a command whose exit code is the pass/fail signal
    #[arg(long = "ref-cmd", requires = "ref_dir", conflicts_with_all = ["ref_file", "ref_note", "ref_report"])]
    pub ref_cmd: Option<String>,
    /// command: the directory to run `--ref-cmd` in
    #[arg(long = "ref-dir")]
    pub ref_dir: Option<String>,
    /// note: an opaque manual reference (always surfaced as asserted/unverified)
    #[arg(long = "ref-note", conflicts_with_all = ["ref_file", "ref_cmd", "ref_report"])]
    pub ref_note: Option<String>,
    /// file_anchor SUGAR (the zero-drift safe path): point at a result artifact a
    /// tool already produces — `--ref-report <PATH> <ANCHOR>` is exactly a
    /// `file_anchor` (no `--ref-anchor` pairing). Prefer this over `--ref-cmd`:
    /// reading a result file has no drift window; re-running a command does.
    #[arg(
        long = "ref-report",
        num_args = 2,
        value_names = ["PATH", "ANCHOR"],
        conflicts_with_all = ["ref_file", "ref_cmd", "ref_note"]
    )]
    pub ref_report: Option<Vec<String>>,
    /// file_anchor acceptance criterion: a numeric bar that turns the resolved
    /// number into an honest Pass/Fail (e.g. `--expect ">=80"` for coverage).
    /// Without it a bare number stays `Unknown` (muster never guesses whether
    /// higher or lower is "good"). Comparators: `>= <= > < ==`. Applies only to
    /// a file_anchor (`--ref-file`/`--ref-report`).
    #[arg(long = "expect", conflicts_with_all = ["ref_cmd", "ref_note"])]
    pub expect: Option<String>,
}

impl RefFlags {
    /// Build the `Ref` these flags describe, or `None` when no ref kind was given.
    /// Returns `Err` only on a half-specified file/command pair clap can't catch.
    pub fn to_ref(&self) -> Result<Option<Ref>, String> {
        // Parse the optional numeric acceptance criterion once (honest error on
        // bad syntax). Only meaningful for a file_anchor; clap already conflicts
        // it with cmd/note.
        let expect = match &self.expect {
            Some(s) => Some(domain::Expectation::parse(s)?),
            None => None,
        };
        if let Some(report) = &self.ref_report {
            // clap's `num_args = 2` guarantees exactly two values when present.
            let path = report
                .first()
                .ok_or("--ref-report requires <PATH> <ANCHOR>")?
                .clone();
            let anchor = report
                .get(1)
                .ok_or("--ref-report requires <PATH> <ANCHOR>")?
                .clone();
            return Ok(Some(Ref::FileAnchor {
                path,
                anchor,
                expect,
            }));
        }
        if let Some(path) = &self.ref_file {
            let anchor = self
                .ref_anchor
                .clone()
                .ok_or("--ref-file requires --ref-anchor")?;
            Ok(Some(Ref::FileAnchor {
                path: path.clone(),
                anchor,
                expect,
            }))
        } else if let Some(cmd) = &self.ref_cmd {
            let dir = self.ref_dir.clone().ok_or("--ref-cmd requires --ref-dir")?;
            Ok(Some(Ref::Command {
                cmd: cmd.clone(),
                dir,
            }))
        } else if let Some(text) = &self.ref_note {
            Ok(Some(Ref::Note { text: text.clone() }))
        } else {
            Ok(None)
        }
    }
}

/// muster — an AI-first ledningssystem: run your management system as a living
/// process map, become certification-ready, and handle incidents. One spine.
#[derive(Parser, Debug)]
#[command(name = "muster", about)]
pub struct Cli {
    /// Output format: text (human-readable, default) or json (machine-readable).
    /// Explicit --output text overrides config json=true or MUSTER_JSON=true.
    #[arg(long, global = true)]
    pub output: Option<OutputFormat>,

    /// Configuration file path
    #[arg(long, global = true)]
    pub config: Option<String>,

    /// Enable verbose output (debug level)
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Disable the audit log file for this run (CKSPEC-OUT-004 audit
    /// logging is on by default; this overrides it for the current run)
    #[arg(long, global = true)]
    pub no_audit: bool,

    #[command(subcommand)]
    pub command: Commands,
}

/// Output format selection (CKSPEC-OUT-002).
/// Matches ckeletin-go convention: --output text|json
#[derive(Debug, Clone, ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Initialize the muster store in this project (creates the data dir)
    Init,
    /// Map intents → commands (no manual required)
    Explain,
    /// Manage processes — the spine (add, show, steps, controls, checks, revise)
    Process(ProcessCmd),
    /// Manage controls — standard-agnostic requirements you must meet
    Control(ControlCmd),
    /// Incident command & control — report, log, close
    Incident(IncidentCmd),
    /// Nonconformities — findings against a process/control (the refuting signal)
    Nonconformity(NonconformityCmd),
    /// Certification-readiness truth-meter over the process graph
    Readiness(ReadinessArgs),

    /// Check connectivity — returns pong (framework worked example)
    Ping,
    /// Print the binary's build identity (version, commit, date, dirty)
    Version,
    /// Emit the machine-readable command catalog (CKSPEC-AGENT-006)
    Catalog,
}

// Generic value-parser for domain enums (FromStr<Err = String>). Keeps clap in
// the cli layer while the enum definitions stay pure-domain (Manifesto #8).
fn parse_enum<T: std::str::FromStr<Err = String>>(s: &str) -> Result<T, String> {
    s.parse::<T>()
}

// ── process ──────────────────────────────────────────────────────────────────

#[derive(Args, Debug)]
pub struct ProcessCmd {
    #[command(subcommand)]
    pub sub: ProcessSub,
}

#[derive(Subcommand, Debug)]
pub enum ProcessSub {
    /// Stand up a new process (a hypothesis; defaults to status=proposed)
    Add {
        id: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        owner: Option<String>,
        #[arg(long)]
        purpose: Option<String>,
    },
    /// Show a process; --tree expands sub-processes (cycle-safe)
    Show {
        id: String,
        #[arg(long)]
        tree: bool,
    },
    /// List all processes (id-sorted)
    List,
    /// Move a process along its hypothesis lifecycle
    SetStatus {
        id: String,
        #[arg(value_parser = parse_enum::<ProcessStatus>)]
        status: ProcessStatus,
    },
    /// Manage a process's steps
    Step(StepCmd),
    /// Link a control so it governs the whole process
    LinkControl { id: String, control_id: String },
    /// Manage a process's risks
    Risk(RiskCmd),
    /// Manage a process's metrics
    Metric(MetricCmd),
    /// Manage / ingest conformance checks (the #9 enforcement seam)
    Check(CheckArgs),
    /// Record a revision — the #10 feedback cycle (append-only, auditable)
    Revise {
        id: String,
        summary: String,
        /// Id of the incident / nonconformity / check that triggered this change
        #[arg(long)]
        because: Option<String>,
    },
    /// Attach evidence to a process
    AttachEvidence {
        id: String,
        #[arg(value_parser = parse_enum::<EvidenceKind>)]
        kind: EvidenceKind,
        value: String,
    },
}

#[derive(Args, Debug)]
pub struct StepCmd {
    #[command(subcommand)]
    pub sub: StepSub,
}

#[derive(Subcommand, Debug)]
pub enum StepSub {
    /// Add an ordered step; --process-ref delegates to a sub-process
    Add {
        id: String,
        #[arg(long)]
        description: String,
        #[arg(long)]
        owner: Option<String>,
        #[arg(long = "control")]
        control: Vec<String>,
        #[arg(long = "process-ref")]
        process_ref: Option<String>,
    },
}

#[derive(Args, Debug)]
pub struct RiskCmd {
    #[command(subcommand)]
    pub sub: RiskSub,
}
#[derive(Subcommand, Debug)]
pub enum RiskSub {
    /// Add a risk to a process
    Add { id: String, risk: String },
}

#[derive(Args, Debug)]
pub struct MetricCmd {
    #[command(subcommand)]
    pub sub: MetricSub,
}
#[derive(Subcommand, Debug)]
pub enum MetricSub {
    /// Add a metric to a process
    Add { id: String, metric: String },
}

/// `process check` has two SPEC forms: `check add <id> --description --enforcement`
/// (create) and `check <id> <check-id> --pass|--fail` (ingest a result). The
/// subcommand-or-args idiom makes both literal invocations work.
#[derive(Args, Debug)]
#[command(args_conflicts_with_subcommands = true, subcommand_negates_reqs = true)]
pub struct CheckArgs {
    #[command(subcommand)]
    pub sub: Option<CheckSub>,
    /// Ingest form: the process id
    pub id: Option<String>,
    /// Ingest form: the check id
    pub check_id: Option<String>,
    /// Record a passing result
    #[arg(long, conflicts_with = "fail")]
    pub pass: bool,
    /// Record a failing result
    #[arg(long)]
    pub fail: bool,
    /// Re-resolve a ref-backed check (refresh its cached resolution)
    #[arg(long, conflicts_with_all = ["pass", "fail"])]
    pub resolve: bool,
    /// Optional evidence for the result: <kind> <value>
    #[arg(long, num_args = 2, value_names = ["KIND", "VALUE"])]
    pub evidence: Option<Vec<String>>,
}

#[derive(Subcommand, Debug)]
pub enum CheckSub {
    /// Create a conformance check on a process. Pass `--ref-*` to derive its
    /// result from an authoritative source (#7) instead of hand-setting it.
    Add {
        id: String,
        #[arg(long)]
        description: String,
        #[arg(long, value_parser = parse_enum::<Enforcement>)]
        enforcement: Enforcement,
        #[command(flatten)]
        ref_flags: RefFlags,
    },
}

// ── control ──────────────────────────────────────────────────────────────────

#[derive(Args, Debug)]
pub struct ControlCmd {
    #[command(subcommand)]
    pub sub: ControlSub,
}

#[derive(Subcommand, Debug)]
pub enum ControlSub {
    /// Define a control (any framework — muster is standard-agnostic). Pass
    /// `--ref-*` to back its title/status by an authoritative source (#7).
    Add {
        id: String,
        /// Human label. Optional when a `--ref-*` backs the control (the title is
        /// then a DERIVED projection of the source; this is a fallback only).
        #[arg(long)]
        title: Option<String>,
        #[arg(long = "clause-ref")]
        clause_ref: Option<String>,
        #[arg(long)]
        applicable: Option<bool>,
        #[command(flatten)]
        ref_flags: RefFlags,
    },
    /// List all controls
    List,
    /// Show a control (derives title/status from its ref on read)
    Show { id: String },
    /// Set a control's implementation status
    SetStatus {
        id: String,
        #[arg(value_parser = parse_enum::<ControlStatus>)]
        status: ControlStatus,
    },
    /// Attach evidence to a control
    AttachEvidence {
        id: String,
        #[arg(value_parser = parse_enum::<EvidenceKind>)]
        kind: EvidenceKind,
        value: String,
    },
    /// Re-resolve a control's ref (refresh the cached resolution). Pass `--all` to
    /// re-resolve every ref-backed control and flag any that silently went
    /// `Unresolved` after a source refactor (the doctor surface).
    Resolve {
        /// The control id to re-resolve (omit when using `--all`)
        id: Option<String>,
        /// Re-resolve every ref-backed control + implementation
        #[arg(long)]
        all: bool,
    },
    /// Link an implementation (N:M) — point it at its own source (#7)
    AddImplementation {
        id: String,
        #[arg(long = "impl-id")]
        impl_id: String,
        #[command(flatten)]
        ref_flags: RefFlags,
    },
    /// Import controls as references from a TOML/JSON requirements manifest
    Import {
        manifest: String,
        #[arg(long, value_parser = parse_enum::<ManifestFormat>)]
        format: Option<ManifestFormat>,
        #[arg(long, default_value = "requirements")]
        prefix: String,
        #[arg(long = "title-field", default_value = "title")]
        title_field: String,
    },
}

/// Manifest format for `control import` (else inferred from the file extension).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManifestFormat {
    Toml,
    Json,
}

impl std::str::FromStr for ManifestFormat {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "toml" => Ok(ManifestFormat::Toml),
            "json" => Ok(ManifestFormat::Json),
            other => Err(format!(
                "invalid format '{other}' — expected one of: toml, json"
            )),
        }
    }
}

// ── incident ─────────────────────────────────────────────────────────────────

#[derive(Args, Debug)]
pub struct IncidentCmd {
    #[command(subcommand)]
    pub sub: IncidentSub,
}

#[derive(Subcommand, Debug)]
pub enum IncidentSub {
    /// Report an incident (muster the team — C2)
    Report {
        id: String,
        #[arg(long)]
        title: String,
        #[arg(long, value_parser = parse_enum::<Severity>)]
        severity: Option<Severity>,
        #[arg(long = "process")]
        process: Option<String>,
    },
    /// List all incidents
    List,
    /// Show an incident
    Show { id: String },
    /// Append a timeline note to an incident
    Log { id: String, note: String },
    /// Close an incident
    Close { id: String },
}

// ── nonconformity ────────────────────────────────────────────────────────────

#[derive(Args, Debug)]
pub struct NonconformityCmd {
    #[command(subcommand)]
    pub sub: NonconformitySub,
}

#[derive(Subcommand, Debug)]
pub enum NonconformitySub {
    /// Raise a nonconformity; --from-incident copies its process_ref
    Raise {
        id: String,
        #[arg(long)]
        description: String,
        #[arg(long = "from-incident")]
        from_incident: Option<String>,
        #[arg(long = "process")]
        process: Option<String>,
        #[arg(long = "control")]
        control: Option<String>,
        #[arg(long, value_parser = parse_enum::<NonconformitySource>)]
        source: Option<NonconformitySource>,
    },
    /// List all nonconformities
    List,
    /// Show a nonconformity
    Show { id: String },
    /// Resolve (close) a nonconformity, optionally recording the corrective action
    Resolve {
        id: String,
        #[arg(long = "corrective-action")]
        corrective_action: Option<String>,
    },
}

// ── readiness ────────────────────────────────────────────────────────────────

#[derive(Args, Debug)]
pub struct ReadinessArgs {
    /// Scope to one process and its reachable sub-graph
    #[arg(long = "process")]
    pub process: Option<String>,
}
