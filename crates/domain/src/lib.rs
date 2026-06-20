//! muster domain — pure entity types, validation, the in-memory aggregate, graph
//! traversal (tree + cycle detection), and readiness computation.
//!
//! No I/O, no clap, no fs (Manifesto #8). The cli crate owns the disk boundary.

pub mod model;
pub mod ping;
pub mod readiness;
pub mod reference;
pub mod store;

pub use model::{
    Check, CheckResult, Control, ControlStatus, DomainError, Enforcement, Evidence, EvidenceKind,
    EvidenceVerdict, Implementation, Incident, IncidentStatus, LogEntry, Nonconformity,
    NonconformitySource, NonconformityStatus, Process, ProcessStatus, Revision, Severity, Step,
    has_verifying_evidence, is_well_formed_url, validate_slug, verify_evidence,
};
pub use readiness::{ControlsSplit, Readiness, readiness, readiness_with};
pub use reference::{
    Comparator, Derived, Expectation, Outcome, Ref, Resolution, drift_profile, is_stale,
    value_to_outcome, value_to_outcome_with_expect,
};
pub use store::{Store, SubNode, TreeStep, TreeView};
