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

use crate::model::{Control, Incident, Nonconformity, Process};
use crate::store::{SCHEMA_VERSION, Store};
use serde::{Deserialize, Serialize};

/// The serde default for `schema_version`: an unversioned (legacy) manifest is
/// read as v1 (#7 SSOT — one schema number, reused from `domain::SCHEMA_VERSION`).
fn default_schema_version() -> u32 {
    SCHEMA_VERSION
}

/// The entire store as one declarative document — every process, control,
/// incident, and nonconformity. The shape `state` emits and `apply` consumes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
