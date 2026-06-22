//! `muster apply <manifest>` — author/update the whole store, declaratively.
//!
//! The inverse of `state`: deserialize the same `StoreDocument` shape and persist
//! it. UPSERT every entity by id (no prune, v3), IDEMPOTENT (a second apply is
//! byte-identical), FAIL-CLOSED (a dangling-anchor / malformed manifest is refused
//! as a WHOLE — the store is left exactly as it was), and `--dry-run` prints the
//! would-be `readiness` verdict WITHOUT mutating. The manifest IS the store shape
//! (#7); only this cli layer touches disk (#8); ref validation precedes the single
//! writer, so a refused apply cannot half-write (#9).

use crate::readiness;
use crate::resolve;
use crate::root::ApplyArgs;
use crate::store;
use crate::view::WithNext;
use domain::StoreDocument;
use infrastructure::output::Output;
use serde::Serialize;
use std::fmt;
use std::io;

type Boxed = Result<(), Box<dyn std::error::Error>>;

/// Per-category upsert counts — the success summary (dual-surface: JSON mirrors
/// the human fields).
#[derive(Serialize)]
struct ApplySummary {
    processes: usize,
    controls: usize,
    incidents: usize,
    nonconformities: usize,
}

impl fmt::Display for ApplySummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "applied: {} process(es), {} control(s), {} incident(s), {} nonconformity(ies) upserted",
            self.processes, self.controls, self.incidents, self.nonconformities
        )
    }
}

pub fn execute(args: ApplyArgs, output: &Output) -> Boxed {
    let dir = store::data_dir();

    // 1. Read + parse the manifest (fail-closed on shape). Honest errors name the
    //    path / the parse failure / the offending field.
    let text = std::fs::read_to_string(&args.manifest).map_err(|e| {
        format!(
            "could not read manifest '{}': {e} — check the path",
            args.manifest
        )
    })?;
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|e| {
        format!(
            "manifest '{}' is not valid JSON: {e} — pass the document `muster state --output json` emits",
            args.manifest
        )
    })?;
    // Envelope unwrap: `state --output json` emits the CKSPEC envelope, so if the
    // value is an object carrying both "command" and "data", take `.data`; else use
    // it whole (a bare document is accepted too). This makes apply(state()) work
    // against the literal bytes `state` emitted.
    let doc_value = match &value {
        serde_json::Value::Object(map)
            if map.contains_key("command") && map.contains_key("data") =>
        {
            value
                .get("data")
                .cloned()
                .expect("checked data key is present")
        }
        _ => value,
    };
    let doc: StoreDocument = serde_json::from_value(doc_value).map_err(|e| {
        format!(
            "manifest '{}' does not match the store shape: {e} — it must be the document `muster state --output json` emits",
            args.manifest
        )
    })?;

    // 2. Build the merged would-be store in memory (no disk writes yet). `apply`
    //    requires an initialized store (honest error otherwise).
    let mut merged = store::load(&dir)?;
    doc.upsert_into(&mut merged);

    // 3. Fail-closed ref validation BEFORE any persist (#9). Because validation
    //    completes fully here and step 4 is the only writer, a refused manifest
    //    leaves the on-disk store byte-for-byte untouched (structural all-or-nothing).
    resolve::validate_store_refs(&merged)?;

    // 4. Branch on --dry-run.
    if args.dry_run {
        // Preview only: render the would-be readiness verdict over `merged` via the
        // SHARED renderer (#7). NO store::save — the store is not mutated.
        readiness::render_for_store(merged, None, output, "apply")?;
        return Ok(());
    }

    // Persist (the single writer). Idempotency is structural: `save` serializes
    // each entity with `to_string_pretty`, so re-applying the same document writes
    // byte-identical files and a subsequent `state` is byte-identical.
    store::save(&dir, &merged)?;
    let summary = ApplySummary {
        processes: doc.processes.len(),
        controls: doc.controls.len(),
        incidents: doc.incidents.len(),
        nonconformities: doc.nonconformities.len(),
    };
    let wrapped = WithNext::new(&summary, "muster state --output json  |  muster readiness");
    output.success("apply", &wrapped, &mut io::stdout())?;
    Ok(())
}
