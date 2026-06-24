//! `muster apply <manifest>` — author/update the whole store, declaratively.
//!
//! The inverse of `state`: deserialize the same `StoreDocument` shape and persist
//! it. UPSERT every entity by id (no prune, v3), IDEMPOTENT (a second apply is
//! byte-identical), FAIL-CLOSED (a dangling-anchor / malformed manifest is refused
//! as a WHOLE — the store is left exactly as it was), and `--dry-run` prints the
//! would-be `readiness` verdict WITHOUT mutating (and WITHOUT gating the exit
//! code — `apply` uses 0/1 only). The manifest IS the store shape
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

const STORE_FIELDS: &[&str] = &[
    "schema_version",
    "processes",
    "controls",
    "incidents",
    "nonconformities",
];

/// Unwrap exactly the JSON envelope emitted by `state --output json`, otherwise
/// leave a bare store document intact. A top-level `data` key beside store fields
/// is ambiguous and rejected; accepting it as an envelope would silently discard
/// siblings such as `controls`.
fn manifest_document_value(value: serde_json::Value) -> Result<serde_json::Value, String> {
    let serde_json::Value::Object(map) = &value else {
        return Ok(value);
    };
    let has_data = map.contains_key("data");
    let has_store_field = STORE_FIELDS.iter().any(|field| map.contains_key(*field));
    if has_data && has_store_field {
        return Err(
            "ambiguous manifest: top-level `data` appears beside store fields; pass either the full `state --output json` envelope or a bare store document, not both"
                .to_string(),
        );
    }
    if has_data && map.contains_key("status") {
        return Ok(map.get("data").expect("checked contains_key").clone());
    }
    Ok(value)
}

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
    // Envelope unwrap (#1): take the CKSPEC envelope's `data`, else the bare value.
    // The final `StoreDocument` parse below still owns wrong-shape / unknown-field
    // errors so serde can name the offending field.
    let doc_value = manifest_document_value(value).map_err(|e| {
        format!(
            "manifest '{}' does not match the store shape: {e} — it must be the document `muster state --output json` emits",
            args.manifest
        )
    })?;
    let doc: StoreDocument = serde_json::from_value(doc_value).map_err(|e| {
        format!(
            "manifest '{}' does not match the store shape: {e} — it must be the document `muster state --output json` emits",
            args.manifest
        )
    })?;

    // Forward-protection (#7): refuse a manifest from a newer binary rather than
    // silently misparsing it. An unversioned manifest defaults to v1 (accepted).
    if doc.schema_version > domain::SCHEMA_VERSION {
        return Err(format!(
            "refusing to apply: manifest '{}' has schema_version {} but this muster understands up to {} — upgrade muster, then retry; the store was left unchanged.",
            args.manifest, doc.schema_version, domain::SCHEMA_VERSION
        )
        .into());
    }

    // 2. Build the merged would-be store in memory (no disk writes yet). `apply`
    //    requires an initialized store (honest error otherwise).
    let mut merged = store::load(&dir)?;
    doc.upsert_into(&mut merged);

    // 3. Fail-closed validation of the FULL matrix BEFORE any persist (#9):
    //    domain-pure id integrity + intra-document refs, then live ref resolution.
    //    Because validation completes fully here and step 4 is the only writer, a
    //    refused manifest leaves the on-disk store byte-for-byte untouched. The
    //    later save is per-file atomic, not a cross-file transaction.
    doc.validate(&merged)?;
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
