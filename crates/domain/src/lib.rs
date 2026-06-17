//! muster domain — pure entity types, validation, the in-memory aggregate, graph
//! traversal (tree + cycle detection), and readiness computation.
//!
//! No I/O, no clap, no fs (Manifesto #8). The cli crate owns the disk boundary.

pub mod model;
pub mod ping;
pub mod readiness;
pub mod store;

pub use model::{
    Check, CheckResult, Control, ControlStatus, DomainError, Enforcement, Evidence, EvidenceKind,
    Incident, IncidentStatus, LogEntry, Nonconformity, NonconformitySource, NonconformityStatus,
    Process, ProcessStatus, Revision, Severity, Step, validate_slug,
};
pub use readiness::{Readiness, readiness};
pub use store::{Store, SubNode, TreeStep, TreeView};
