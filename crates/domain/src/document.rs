//! `StoreDocument` — the one schema in and out (Manifesto #7 SSOT).
//!
//! The manifest IS the store shape. There is no second, hand-maintained format:
//! `state` serializes a `StoreDocument` *out* of the `Store`, and `apply`
//! deserializes the same shape and merges it *in*. Pure data — no I/O, no stdout
//! (#8); the cli store layer owns disk, the cli resolve layer owns ref validation.
//!
//! Arrays, not maps: every entity already carries its own `id`, and the source
//! `Store` is `BTreeMap`-keyed, so `.values()` is already id-sorted → the document
//! is deterministic and diffable (#7, AX). `#[serde(default)]` on each field keeps
//! a manifest that omits an empty category readable.

use crate::model::{Control, DomainError, Incident, Nonconformity, Process, validate_slug};
use crate::store::{SCHEMA_VERSION, Store};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// The serde default for `schema_version`: an unversioned (legacy) manifest is
/// read as v1 (#7 SSOT — one schema number, reused from `domain::SCHEMA_VERSION`).
fn default_schema_version() -> u32 {
    SCHEMA_VERSION
}

/// The entire store as one declarative document — every process, control,
/// incident, and nonconformity. The shape `state` emits and `apply` consumes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoreDocument {
    /// The on-disk schema version (#7). Declared first so `state` output leads
    /// with it deterministically; an unversioned manifest defaults to v1.
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub processes: Vec<Process>,
    #[serde(default)]
    pub controls: Vec<Control>,
    #[serde(default)]
    pub incidents: Vec<Incident>,
    #[serde(default)]
    pub nonconformities: Vec<Nonconformity>,
}

impl Default for StoreDocument {
    /// Hand-written (not derived) so an empty document still carries the current
    /// `SCHEMA_VERSION` — a derived `Default` would wrongly stamp `0`.
    fn default() -> Self {
        StoreDocument {
            schema_version: SCHEMA_VERSION,
            processes: Vec::new(),
            controls: Vec::new(),
            incidents: Vec::new(),
            nonconformities: Vec::new(),
        }
    }
}

impl From<&Store> for StoreDocument {
    /// Serialize the whole store (the `state` direction). id-sorted (the source
    /// `BTreeMap`s already are), deterministic, with NO ref re-resolution and NO
    /// mutation — `state` is structurally read-only.
    fn from(s: &Store) -> Self {
        StoreDocument {
            schema_version: SCHEMA_VERSION,
            processes: s.processes.values().cloned().collect(),
            controls: s.controls.values().cloned().collect(),
            incidents: s.incidents.values().cloned().collect(),
            nonconformities: s.nonconformities.values().cloned().collect(),
        }
    }
}

impl StoreDocument {
    /// Domain-pure trust-boundary validation of the manifest against the `merged`
    /// (manifest ∪ existing store) result — the structural sibling of the cli's
    /// `validate_store_refs` (which does live ref I/O). Fail-closed, naming the
    /// offending entity and the fix (#9, #1):
    /// 1. **id integrity** — every entity id is a valid slug and unique within its
    ///    category (a duplicate id in the manifest is silent last-write-wins; the
    ///    interactive path rejects it, so `apply` must too).
    /// 2. **intra-document refs** — every `process.controls` / `step.process_ref` /
    ///    `step.controls` / `incident.process_ref` / `nonconformity.process_ref` /
    ///    `nonconformity.control_ref` link resolves in `merged`.
    pub fn validate(&self, merged: &Store) -> Result<(), DomainError> {
        // 1. id integrity (slug + per-category uniqueness over the manifest Vecs).
        check_ids("process", self.processes.iter().map(|p| &p.id))?;
        check_ids("control", self.controls.iter().map(|c| &c.id))?;
        check_ids("incident", self.incidents.iter().map(|i| &i.id))?;
        check_ids("nonconformity", self.nonconformities.iter().map(|n| &n.id))?;

        // 2. intra-document refs against the merged store.
        for p in &self.processes {
            for cid in &p.controls {
                require_control(merged, cid, format_args!("process '{}' controls", p.id))?;
            }
            for s in &p.steps {
                if let Some(r) = &s.process_ref {
                    require_process(merged, r, format_args!("a step of process '{}'", p.id))?;
                }
                for cid in &s.controls {
                    require_control(merged, cid, format_args!("a step of process '{}'", p.id))?;
                }
            }
        }
        for i in &self.incidents {
            if let Some(r) = &i.process_ref {
                require_process(merged, r, format_args!("incident '{}'", i.id))?;
            }
        }
        for n in &self.nonconformities {
            if let Some(r) = &n.process_ref {
                require_process(merged, r, format_args!("nonconformity '{}'", n.id))?;
            }
            if let Some(r) = &n.control_ref {
                require_control(merged, r, format_args!("nonconformity '{}'", n.id))?;
            }
        }
        Ok(())
    }

    /// Merge this document into a store (the `apply` direction): create-or-replace
    /// every entity by id (UPSERT). v3 does NOT prune entities absent from the
    /// document — `apply` is additive, which keeps the round-trip exact. Pure: it
    /// does only the structural merge; ref validation is the cli's job (#8).
    pub fn upsert_into(&self, store: &mut Store) {
        for p in &self.processes {
            store.processes.insert(p.id.clone(), p.clone());
        }
        for c in &self.controls {
            store.controls.insert(c.id.clone(), c.clone());
        }
        for i in &self.incidents {
            store.incidents.insert(i.id.clone(), i.clone());
        }
        for n in &self.nonconformities {
            store.nonconformities.insert(n.id.clone(), n.clone());
        }
    }
}

/// Reject a non-slug or duplicate id within one manifest category — naming the
/// offending id (the merged BTreeMap has already deduped, so the scan is over the
/// manifest's `Vec`s where the duplicate is still visible).
fn check_ids<'a>(
    kind: &'static str,
    ids: impl Iterator<Item = &'a String>,
) -> Result<(), DomainError> {
    let mut seen = BTreeSet::new();
    for id in ids {
        validate_slug(id)?;
        if !seen.insert(id.as_str()) {
            return Err(DomainError::DuplicateId {
                kind,
                id: id.clone(),
            });
        }
    }
    Ok(())
}

/// Refuse a dangling intra-document control reference, naming both the dangling
/// control id and the referring entity (so the offender is unambiguous, #1).
fn require_control(
    merged: &Store,
    id: &str,
    referrer: std::fmt::Arguments,
) -> Result<(), DomainError> {
    if merged.controls.contains_key(id) {
        Ok(())
    } else {
        Err(DomainError::mref(
            "control",
            id,
            format!(
                "referenced by {referrer} but present in neither the manifest nor the store — add the control or remove the reference"
            ),
        ))
    }
}

/// Refuse a dangling intra-document process reference (mirror of [`require_control`]).
fn require_process(
    merged: &Store,
    id: &str,
    referrer: std::fmt::Arguments,
) -> Result<(), DomainError> {
    if merged.processes.contains_key(id) {
        Ok(())
    } else {
        Err(DomainError::mref(
            "process",
            id,
            format!(
                "referenced by {referrer} but present in neither the manifest nor the store — add the process or remove the reference"
            ),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ControlStatus, NonconformitySource, Severity};

    fn seeded() -> Store {
        let mut s = Store::default();
        // Insert out of id-order to prove `from` re-sorts via BTreeMap iteration.
        s.add_process("zeta", "Zeta", None, None).unwrap();
        s.add_process("alpha", "Alpha", None, None).unwrap();
        s.add_control("ctrl-b", "B", None, true).unwrap();
        s.add_control("ctrl-a", "A", None, true).unwrap();
        s.report_incident("inc-1", "Outage", Severity::High, None)
            .unwrap();
        s.raise_nonconformity(
            "nc-1",
            "slow",
            NonconformitySource::Manual,
            None,
            None,
            None,
        )
        .unwrap();
        s
    }

    fn doc_control(id: &str) -> Control {
        Control {
            id: id.into(),
            title: "C".into(),
            clause_ref: None,
            applicable: true,
            status: ControlStatus::NotStarted,
            evidence: vec![],
            r#ref: None,
            resolved: None,
            implementations: vec![],
        }
    }

    /// Build the merged store the cli would compute, then validate against it.
    fn validate_self(doc: &StoreDocument) -> Result<(), DomainError> {
        let mut merged = Store::default();
        doc.upsert_into(&mut merged);
        doc.validate(&merged)
    }

    #[test]
    fn validate_accepts_a_self_consistent_document() {
        let s = seeded();
        let doc = StoreDocument::from(&s);
        doc.validate(&s)
            .expect("a self-consistent document validates");
    }

    #[test]
    fn validate_rejects_a_duplicate_id_within_a_category() {
        let doc = StoreDocument {
            controls: vec![doc_control("dup"), doc_control("dup")],
            ..StoreDocument::default()
        };
        match validate_self(&doc) {
            Err(DomainError::DuplicateId { kind, id }) => {
                assert_eq!(kind, "control");
                assert_eq!(id, "dup");
            }
            other => panic!("expected DuplicateId, got {other:?}"),
        }
    }

    #[test]
    fn validate_rejects_an_invalid_slug_id() {
        let doc = StoreDocument {
            controls: vec![doc_control("Bad Id")],
            ..StoreDocument::default()
        };
        assert!(matches!(
            validate_self(&doc),
            Err(DomainError::InvalidSlug(_))
        ));
    }

    #[test]
    fn validate_rejects_dangling_process_controls_link() {
        let mut p = Process::new("p1".into(), "P".into());
        p.controls = vec!["ghost".into()];
        let doc = StoreDocument {
            processes: vec![p],
            ..StoreDocument::default()
        };
        match validate_self(&doc) {
            Err(DomainError::MissingReference { kind, id, fix }) => {
                assert_eq!(kind, "control");
                assert_eq!(id, "ghost");
                assert!(fix.contains("p1"), "fix must name the referrer: {fix}");
            }
            other => panic!("expected MissingReference, got {other:?}"),
        }
    }

    #[test]
    fn validate_rejects_dangling_step_process_ref() {
        let mut p = Process::new("p1".into(), "P".into());
        p.steps = vec![crate::model::Step {
            n: 1,
            description: "do".into(),
            owner: None,
            controls: vec![],
            process_ref: Some("ghost".into()),
        }];
        let doc = StoreDocument {
            processes: vec![p],
            ..StoreDocument::default()
        };
        assert!(matches!(
            validate_self(&doc),
            Err(DomainError::MissingReference {
                kind: "process",
                ..
            })
        ));
    }

    #[test]
    fn validate_rejects_dangling_nonconformity_control_ref() {
        let nc = Nonconformity {
            id: "nc1".into(),
            source: NonconformitySource::Manual,
            process_ref: None,
            control_ref: Some("ghost".into()),
            description: "d".into(),
            corrective_action: None,
            status: crate::model::NonconformityStatus::Open,
        };
        let doc = StoreDocument {
            nonconformities: vec![nc],
            ..StoreDocument::default()
        };
        match validate_self(&doc) {
            Err(DomainError::MissingReference { kind, id, .. }) => {
                assert_eq!(kind, "control");
                assert_eq!(id, "ghost");
            }
            other => panic!("expected MissingReference, got {other:?}"),
        }
    }

    #[test]
    fn validate_accepts_intra_document_ref_resolved_within_the_manifest() {
        // A process links a control defined in the SAME manifest → resolves.
        let mut p = Process::new("p1".into(), "P".into());
        p.controls = vec!["c1".into()];
        let doc = StoreDocument {
            processes: vec![p],
            controls: vec![doc_control("c1")],
            ..StoreDocument::default()
        };
        validate_self(&doc).expect("a ref satisfied within the manifest validates");
    }

    #[test]
    fn from_store_collects_every_category_id_sorted() {
        let doc = StoreDocument::from(&seeded());
        assert_eq!(
            doc.processes.iter().map(|p| &p.id).collect::<Vec<_>>(),
            vec!["alpha", "zeta"],
            "processes must be id-sorted"
        );
        assert_eq!(
            doc.controls.iter().map(|c| &c.id).collect::<Vec<_>>(),
            vec!["ctrl-a", "ctrl-b"]
        );
        assert_eq!(doc.incidents.len(), 1);
        assert_eq!(doc.nonconformities.len(), 1);
    }

    #[test]
    fn upsert_creates_replaces_by_id_and_never_prunes() {
        let mut store = Store::default();
        // A pre-existing entity absent from the doc must SURVIVE (no prune).
        store
            .add_process("survivor", "Survivor", None, None)
            .unwrap();
        // And a same-id entity must be REPLACED, not duplicated.
        store
            .add_control("ctrl-a", "stale title", None, true)
            .unwrap();

        let mut doc = StoreDocument::from(&seeded());
        // Mutate the doc's ctrl-a so we can prove replacement happened.
        doc.controls[0].status = ControlStatus::Implemented;
        doc.upsert_into(&mut store);

        // Survivor is untouched (no prune).
        assert!(store.processes.contains_key("survivor"));
        // ctrl-a replaced by the doc's version (status carried over).
        assert_eq!(
            store.control("ctrl-a").unwrap().status,
            ControlStatus::Implemented
        );
        // No duplication — exactly the two doc controls plus none extra.
        assert_eq!(store.controls.len(), 2);
        // The doc's other entities were inserted.
        assert!(store.processes.contains_key("alpha"));
    }

    #[test]
    fn in_memory_round_trip_is_a_fixpoint() {
        // build → serialize → deserialize → upsert into an empty store reproduces
        // an equal Store (the in-memory fixpoint that underpins the round-trip).
        let original = seeded();
        let doc = StoreDocument::from(&original);
        let json = serde_json::to_string(&doc).unwrap();
        let parsed: StoreDocument = serde_json::from_str(&json).unwrap();
        let mut rebuilt = Store::default();
        parsed.upsert_into(&mut rebuilt);
        assert_eq!(rebuilt, original, "apply(state(store)) must equal store");
    }

    #[test]
    fn omitted_categories_default_to_empty() {
        // A manifest that omits empty categories is readable (serde default).
        let doc: StoreDocument = serde_json::from_str(r#"{"processes":[]}"#).unwrap();
        assert!(doc.processes.is_empty());
        assert!(doc.controls.is_empty());
        assert!(doc.incidents.is_empty());
        assert!(doc.nonconformities.is_empty());
        // An unversioned manifest defaults to v1 (#7 SSOT, forward-protection).
        assert_eq!(doc.schema_version, SCHEMA_VERSION);
    }

    #[test]
    fn from_store_stamps_the_schema_version() {
        let doc = StoreDocument::from(&seeded());
        assert_eq!(doc.schema_version, SCHEMA_VERSION);
    }

    #[test]
    fn default_carries_schema_version_not_zero() {
        // A hand-written Default must set the version (a derived Default would yield 0).
        assert_eq!(StoreDocument::default().schema_version, SCHEMA_VERSION);
    }
}
