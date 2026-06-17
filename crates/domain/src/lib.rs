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
    Implementation, Incident, IncidentStatus, LogEntry, Nonconformity, NonconformitySource,
    NonconformityStatus, Process, ProcessStatus, Revision, Severity, Step, validate_slug,
};
pub use readiness::{Readiness, readiness};
pub use reference::{Derived, Outcome, Ref, Resolution, is_stale, value_to_outcome};
pub use store::{Store, SubNode, TreeStep, TreeView};
