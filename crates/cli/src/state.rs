//! `muster state` — read the entire store as one declarative document (#7 SSOT).
//!
//! Serializes the whole `domain::Store` (every process, control, incident,
//! nonconformity) to a single `StoreDocument`. `--output json` is the authoritative
//! shape; human mode mirrors the same fields with a per-category summary. `state`
//! is structurally **read-only** — it loads, serializes, and renders; it NEVER
//! calls `store::save` and NEVER re-resolves refs.

use crate::store;
use crate::view::WithNext;
use domain::StoreDocument;
use infrastructure::output::Output;
use serde::Serialize;
use std::fmt;
use std::io;

type Boxed = Result<(), Box<dyn std::error::Error>>;

/// The dual-surface view of the whole store. JSON serializes transparently as the
/// `StoreDocument` (the authoritative shape `apply` consumes); human mode renders
/// a faithful per-category summary of the SAME fields (no human-only data, no
/// markdown in JSON).
#[derive(Serialize)]
#[serde(transparent)]
struct StateView<'a> {
    doc: &'a StoreDocument,
}

impl fmt::Display for StateView<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let d = self.doc;
        writeln!(
            f,
            "store: {} process(es), {} control(s), {} incident(s), {} nonconformity(ies)",
            d.processes.len(),
            d.controls.len(),
            d.incidents.len(),
            d.nonconformities.len()
        )?;
        if !d.processes.is_empty() {
            writeln!(f, "  processes:")?;
            for p in &d.processes {
                writeln!(f, "    {} — {}", p.id, p.name)?;
            }
        }
        if !d.controls.is_empty() {
            writeln!(f, "  controls:")?;
            for c in &d.controls {
                writeln!(f, "    {} — {}", c.id, c.title)?;
            }
        }
        if !d.incidents.is_empty() {
            writeln!(f, "  incidents:")?;
            for i in &d.incidents {
                writeln!(f, "    {} — {}", i.id, i.title)?;
            }
        }
        if !d.nonconformities.is_empty() {
            writeln!(f, "  nonconformities:")?;
            for n in &d.nonconformities {
                writeln!(f, "    {} — {}", n.id, n.description)?;
            }
        }
        Ok(())
    }
}

pub fn execute(output: &Output) -> Boxed {
    let s = store::load(&store::data_dir())?;
    let doc = StoreDocument::from(&s);
    let view = StateView { doc: &doc };
    // Read-only: NO store::save. The "Next:" hint is JSON-transparent (WithNext),
    // so `data` stays a clean serialization of the StoreDocument.
    let wrapped = WithNext::new(&view, "muster apply <manifest>  (preview with --dry-run)");
    output.success("state", &wrapped, &mut io::stdout())?;
    Ok(())
}
