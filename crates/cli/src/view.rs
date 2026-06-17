//! Rendering helpers that preserve the dual-surface contract (Manifesto #7).
//!
//! `WithNext` wraps any entity so the **human** surface ends with a "Next:"
//! suggestion (#6 hand the next action) while the **JSON** surface stays a
//! transparent serialization of the entity — same facts, no human-only strings
//! leaking into `data`. `Listing` does the same for collections: JSON is a plain
//! array of entities; human is a deterministic summary list.

use domain::{Control, Incident, Nonconformity, Process};
use serde::{Serialize, Serializer};
use std::fmt;

/// An entity plus a human-only "Next:" hint. Serializes transparently as the
/// inner entity (the hint is guidance, not data — it never enters `data`).
pub struct WithNext<'a, T> {
    inner: &'a T,
    next: String,
}

impl<'a, T> WithNext<'a, T> {
    pub fn new(inner: &'a T, next: impl Into<String>) -> Self {
        Self {
            inner,
            next: next.into(),
        }
    }
}

impl<T: Serialize> Serialize for WithNext<'_, T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.inner.serialize(s)
    }
}

impl<T: fmt::Display> fmt::Display for WithNext<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Inner Display ends each line with a newline; append the hint as a
        // trailing line. Output::success adds the final newline.
        write!(f, "{}Next: {}", self.inner, self.next)
    }
}

/// A compact one-line summary of an entity for `list` human output.
pub trait Summary {
    fn summary_line(&self) -> String;
}

impl Summary for Process {
    fn summary_line(&self) -> String {
        format!("{}  {}  [{}]", self.id, self.name, self.status)
    }
}
impl Summary for Control {
    fn summary_line(&self) -> String {
        format!("{}  {}  [{}]", self.id, self.title, self.status)
    }
}
impl Summary for Incident {
    fn summary_line(&self) -> String {
        format!(
            "{}  {}  [{} / {}]",
            self.id, self.title, self.severity, self.status
        )
    }
}
impl Summary for Nonconformity {
    fn summary_line(&self) -> String {
        format!(
            "{}  {}  [{} / {}]",
            self.id, self.description, self.source, self.status
        )
    }
}

/// A list view: JSON is a plain array of entities; human is a summary list with
/// a count and a "Next:" hint.
pub struct Listing<'a, T> {
    items: Vec<&'a T>,
    kind: &'static str,
    next: String,
}

impl<'a, T> Listing<'a, T> {
    pub fn new(items: Vec<&'a T>, kind: &'static str, next: impl Into<String>) -> Self {
        Self {
            items,
            kind,
            next: next.into(),
        }
    }
}

impl<T: Serialize> Serialize for Listing<'_, T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.items.serialize(s)
    }
}

impl<T: Summary> fmt::Display for Listing<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.items.is_empty() {
            writeln!(f, "no {}s yet", self.kind)?;
        } else {
            for item in &self.items {
                writeln!(f, "{}", item.summary_line())?;
            }
            writeln!(f, "{} {}(s)", self.items.len(), self.kind)?;
        }
        write!(f, "Next: {}", self.next)
    }
}
