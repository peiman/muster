//! Persistence — the disk boundary (Manifesto #8: domain stays pure, this layer
//! owns I/O). One JSON file per entity, git-diffable, no database (#4 minimal).
//!
//! Data-dir resolution is the single source of truth (#7): `MUSTER_DATA_DIR` if
//! set, else `./.muster`. Every command loads the four entity dirs into the
//! in-memory `domain::Store`, calls a pure domain op, then saves.

use domain::{Control, Incident, Nonconformity, Process, Store};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const ENV_DATA_DIR: &str = "MUSTER_DATA_DIR";
const DEFAULT_DATA_DIR: &str = "./.muster";
const ENV_FRESHNESS: &str = "MUSTER_FRESHNESS_SECS";
const DEFAULT_FRESHNESS_SECS: i64 = 86_400;
/// Opt-in: serve a *cached* command-ref verdict instead of re-running it live.
/// Default OFF — the honest default re-resolves command refs live on every read
/// (Manifesto #9: a stale verdict is *structurally* unable to show green; there
/// is no cache window for it to outlive reality). Turn this on ONLY for genuinely
/// expensive commands, accepting that the verdict goes `Stale` past the freshness
/// bound and the resolved age is always surfaced.
const ENV_CMD_CACHE: &str = "MUSTER_CMD_CACHE";
/// Opt-in source-freshness bound (seconds). When set, a `file_anchor` whose
/// pointed-at artifact's mtime age exceeds it is flagged stale-by-source in
/// `readiness` (a live `met` from an un-regenerated file is not fresh coverage).
/// Default UNSET ⇒ `None` ⇒ no source-age gating (today's behavior, b2).
const ENV_SOURCE_FRESHNESS: &str = "MUSTER_SOURCE_FRESHNESS_SECS";
const MANIFEST: &str = "manifest.json";
use domain::SCHEMA_VERSION;

const SUBDIRS: [(&str, EntityKind); 4] = [
    ("processes", EntityKind::Process),
    ("controls", EntityKind::Control),
    ("incidents", EntityKind::Incident),
    ("nonconformities", EntityKind::Nonconformity),
];

#[derive(Clone, Copy)]
enum EntityKind {
    Process,
    Control,
    Incident,
    Nonconformity,
}

/// Errors at the disk boundary. `Display` names the corrective action (#3).
#[derive(thiserror::Error, Debug)]
pub enum StoreError {
    #[error("store not initialized — run: muster init")]
    NotInitialized,
    #[error("filesystem error at {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("could not parse {path} (corrupt store?): {source}")]
    Parse {
        path: String,
        #[source]
        source: serde_json::Error,
    },
}

/// Resolve the data dir: `MUSTER_DATA_DIR` env var, else `./.muster`.
pub fn data_dir() -> PathBuf {
    match std::env::var(ENV_DATA_DIR) {
        Ok(v) if !v.is_empty() => PathBuf::from(v),
        _ => PathBuf::from(DEFAULT_DATA_DIR),
    }
}

fn io_err(path: &Path, source: std::io::Error) -> StoreError {
    StoreError::Io {
        path: path.display().to_string(),
        source,
    }
}

pub fn is_initialized(dir: &Path) -> bool {
    dir.join(MANIFEST).is_file()
}

/// Create the store layout: entity dirs + `manifest.json`. Idempotent.
pub fn init(dir: &Path) -> Result<(), StoreError> {
    std::fs::create_dir_all(dir).map_err(|e| io_err(dir, e))?;
    for (sub, _) in SUBDIRS {
        let p = dir.join(sub);
        std::fs::create_dir_all(&p).map_err(|e| io_err(&p, e))?;
    }
    let manifest = dir.join(MANIFEST);
    let body = serde_json::json!({ "schema_version": SCHEMA_VERSION });
    let text = serde_json::to_string_pretty(&body).expect("manifest serializes");
    std::fs::write(&manifest, format!("{text}\n")).map_err(|e| io_err(&manifest, e))?;
    Ok(())
}

/// Load the four entity dirs into the in-memory aggregate.
pub fn load(dir: &Path) -> Result<Store, StoreError> {
    if !is_initialized(dir) {
        return Err(StoreError::NotInitialized);
    }
    let mut store = Store::default();
    for (sub, kind) in SUBDIRS {
        let subdir = dir.join(sub);
        if !subdir.is_dir() {
            continue;
        }
        let entries = std::fs::read_dir(&subdir).map_err(|e| io_err(&subdir, e))?;
        // Collect + sort paths so load order is deterministic.
        let mut paths: Vec<PathBuf> = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|e| io_err(&subdir, e))?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                paths.push(path);
            }
        }
        paths.sort();
        for path in paths {
            let text = std::fs::read_to_string(&path).map_err(|e| io_err(&path, e))?;
            let parse_err = |source| StoreError::Parse {
                path: path.display().to_string(),
                source,
            };
            match kind {
                EntityKind::Process => {
                    let p: Process = serde_json::from_str(&text).map_err(parse_err)?;
                    store.processes.insert(p.id.clone(), p);
                }
                EntityKind::Control => {
                    let c: Control = serde_json::from_str(&text).map_err(parse_err)?;
                    store.controls.insert(c.id.clone(), c);
                }
                EntityKind::Incident => {
                    let i: Incident = serde_json::from_str(&text).map_err(parse_err)?;
                    store.incidents.insert(i.id.clone(), i);
                }
                EntityKind::Nonconformity => {
                    let n: Nonconformity = serde_json::from_str(&text).map_err(parse_err)?;
                    store.nonconformities.insert(n.id.clone(), n);
                }
            }
        }
    }
    Ok(store)
}

/// Serialize one entity to a *temp* file `<sub>/<id>.json.tmp` (byte-identical to
/// the final content), returning the `(temp, final)` pair to be renamed in the
/// commit phase. A serialize or write failure here aborts before any live file is
/// touched (#3 all-or-nothing).
fn stage_entity<T: serde::Serialize>(
    dir: &Path,
    sub: &str,
    id: &str,
    value: &T,
) -> Result<(PathBuf, PathBuf), StoreError> {
    let final_path = dir.join(sub).join(format!("{id}.json"));
    let temp_path = dir.join(sub).join(format!("{id}.json.tmp"));
    let text = serde_json::to_string_pretty(value).map_err(|source| StoreError::Parse {
        path: final_path.display().to_string(),
        source,
    })?;
    std::fs::write(&temp_path, format!("{text}\n")).map_err(|e| io_err(&temp_path, e))?;
    Ok((temp_path, final_path))
}

/// Persist every entity to its `<id>.json` (pretty, stable key order via the
/// struct field order + BTreeMap iteration). The write is **atomic** (#3): every
/// entity is serialized to a `.json.tmp` first (any serialize/IO error aborts
/// before a single live file is touched), then the temps are renamed into place,
/// so a mid-write `ENOSPC`/interruption cannot tear a file or half-write the
/// store. `load()` ignores non-`.json` files, so a temp is invisible to readers
/// even between phases. (Per #4 this is temp+rename, not a cross-file transaction.)
pub fn save(dir: &Path, store: &Store) -> Result<(), StoreError> {
    // Phase 1: serialize + stage all entities to temp files.
    let mut staged: Vec<(PathBuf, PathBuf)> = Vec::new();
    let stage = stage_map(dir, "processes", &store.processes, &mut staged)
        .and_then(|()| stage_map(dir, "controls", &store.controls, &mut staged))
        .and_then(|()| stage_map(dir, "incidents", &store.incidents, &mut staged))
        .and_then(|()| stage_map(dir, "nonconformities", &store.nonconformities, &mut staged));
    if let Err(e) = stage {
        // Best-effort cleanup of temps already written; no live file changed.
        for (temp, _) in &staged {
            let _ = std::fs::remove_file(temp);
        }
        return Err(e);
    }
    // Phase 2: commit — rename each temp into place (atomic per file).
    for (temp, final_path) in &staged {
        std::fs::rename(temp, final_path).map_err(|e| io_err(final_path, e))?;
    }
    Ok(())
}

fn stage_map<T: serde::Serialize>(
    dir: &Path,
    sub: &str,
    map: &BTreeMap<String, T>,
    staged: &mut Vec<(PathBuf, PathBuf)>,
) -> Result<(), StoreError> {
    for (id, value) in map {
        staged.push(stage_entity(dir, sub, id, value)?);
    }
    Ok(())
}

/// Current UTC timestamp as RFC-3339 (`YYYY-MM-DDThh:mm:ssZ`). Computed at the
/// cli boundary and passed *into* domain ops so the domain stays clock-free.
pub fn now_iso() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = i64::try_from(secs / 86_400).unwrap_or(0);
    let rem = secs % 86_400;
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// The freshness bound (seconds) for cached `command`-ref resolutions, from
/// `MUSTER_FRESHNESS_SECS` (default 86400 = one day). `0` ⇒ never trust a served
/// cache (the deterministic SC-7 staleness hook).
pub fn freshness_secs() -> i64 {
    match std::env::var(ENV_FRESHNESS) {
        Ok(v) => v.trim().parse::<i64>().unwrap_or(DEFAULT_FRESHNESS_SECS),
        Err(_) => DEFAULT_FRESHNESS_SECS,
    }
}

/// The one-line warning surfaced (by `readiness` and the `control resolve --all`
/// doctor surface) when command-cache mode is on — SSOT so both surfaces say the
/// same thing (#7). Command-ref verdicts are then served from a cache and may be
/// stale; the honest default re-resolves live.
pub const CMD_CACHE_WARNING: &str = "⚠ command-cache mode is ON (MUSTER_CMD_CACHE) — command-ref verdicts are served from a cache and may be stale; unset it for live re-resolution.";

/// Whether command refs serve a cached verdict (opt-in, default OFF). When off
/// (the honest default), command refs re-resolve live on read — no drift window.
/// Accepts `1`/`true`/`yes`/`on` (case-insensitive) as enabling values.
pub fn cmd_cache_enabled() -> bool {
    match std::env::var(ENV_CMD_CACHE) {
        Ok(v) => matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Err(_) => false,
    }
}

/// The opt-in source-freshness bound (seconds) from `MUSTER_SOURCE_FRESHNESS_SECS`,
/// or `None` when unset/blank/unparseable (no source-age gating — the default).
/// A non-positive value also disables gating (an explicit "don't gate"). Passed
/// into `readiness` so a `file_anchor` reading an un-regenerated artifact past
/// the bound is honestly flagged stale-by-source (b2).
pub fn source_freshness_secs() -> Option<i64> {
    match std::env::var(ENV_SOURCE_FRESHNESS) {
        Ok(v) => v.trim().parse::<i64>().ok().filter(|n| *n > 0),
        Err(_) => None,
    }
}

/// Parse an RFC-3339 `YYYY-MM-DDThh:mm:ssZ` timestamp (as produced by `now_iso`)
/// back to epoch seconds. The inverse of `now_iso` — kept next to it (SSOT, #7).
/// Returns `None` for any shape `now_iso` would never emit.
pub fn parse_iso_to_epoch(ts: &str) -> Option<i64> {
    // Expect exactly "YYYY-MM-DDThh:mm:ssZ" (20 chars).
    let b = ts.as_bytes();
    if b.len() != 20
        || b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b'T'
        || b[13] != b':'
        || b[16] != b':'
        || b[19] != b'Z'
    {
        return None;
    }
    let num = |s: &str| s.parse::<i64>().ok();
    let y = num(&ts[0..4])?;
    let mo = num(&ts[5..7])?;
    let d = num(&ts[8..10])?;
    let h = num(&ts[11..13])?;
    let mi = num(&ts[14..16])?;
    let s = num(&ts[17..19])?;
    let days = days_from_civil(y, mo, d);
    Some(days * 86_400 + h * 3600 + mi * 60 + s)
}

/// (year, month, day) → days since 1970-01-01. Howard Hinnant's `days_from_civil`,
/// the inverse of `civil_from_days`. Computed entirely in `i64`.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146_097 + doe - 719_468
}

/// Days-since-epoch → (year, month, day). Howard Hinnant's `civil_from_days`,
/// computed entirely in `i64`. The final `month`/`day` are mathematically
/// bounded to [1,12] / [1,31], so the narrowing casts are exact.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_from_days_known_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(18_993), (2022, 1, 1));
    }

    #[test]
    fn parse_iso_to_epoch_inverts_now_iso_shape() {
        // Known epoch → civil → string → epoch round-trips.
        assert_eq!(parse_iso_to_epoch("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(
            parse_iso_to_epoch("2022-01-01T00:00:00Z"),
            Some(18_993 * 86_400)
        );
        // A real now_iso parses and the days component matches civil_from_days.
        let ts = now_iso();
        let epoch = parse_iso_to_epoch(&ts).expect("now_iso parses");
        let (y, m, d) = civil_from_days(epoch / 86_400);
        assert_eq!(
            format!("{y:04}-{m:02}-{d:02}"),
            &ts[0..10],
            "epoch round-trip mismatch"
        );
        // Bad shapes return None.
        assert!(parse_iso_to_epoch("not-a-timestamp").is_none());
        assert!(parse_iso_to_epoch("2022-01-01 00:00:00").is_none());
    }

    /// The atomic-save anchor (#3 all-or-nothing, #9 enforced not honor-system):
    /// a `save` that FAILS mid-write must not tear or half-write an entity that
    /// already exists on disk. We force a phase-1 staging failure by pre-creating
    /// `<id>.json.tmp` as a *directory* (so the temp `std::fs::write` errors), then
    /// attempt to overwrite the live entity with new content. Under temp-then-rename
    /// the live `<id>.json` is never touched (this test is GREEN); under a non-atomic
    /// direct write the live file is clobbered before the failure surfaces (RED) — so
    /// this is the genuine guard the `*.tmp`-leftover test could not provide.
    #[test]
    fn a_failed_save_leaves_a_preexisting_entity_byte_unchanged() {
        use domain::{Control, ControlStatus};
        use tempfile::TempDir;

        let mk = |title: &str| {
            let mut store = Store::default();
            store.controls.insert(
                "c-one".to_string(),
                Control {
                    id: "c-one".to_string(),
                    title: title.to_string(),
                    clause_ref: None,
                    applicable: true,
                    status: ControlStatus::NotStarted,
                    evidence: vec![],
                    r#ref: None,
                    resolved: None,
                    implementations: vec![],
                },
            );
            store
        };

        let tmp = TempDir::new().unwrap();
        let dir = tmp.path();
        init(dir).expect("init store layout");

        // 1. Persist the entity with its original content.
        save(dir, &mk("original")).expect("first save succeeds");
        let live = dir.join("controls").join("c-one.json");
        let original_bytes = std::fs::read(&live).expect("read original entity");

        // 2. Block phase-1 staging for this id: a directory at `<id>.json.tmp` makes
        //    the temp write fail before any live file could be renamed.
        std::fs::create_dir(dir.join("controls").join("c-one.json.tmp")).expect("block tmp path");

        // 3. Attempt to overwrite with NEW content — this must fail-closed.
        let result = save(dir, &mk("MUTATED"));

        // The anchor: the pre-existing live file is byte-for-byte unchanged.
        assert_eq!(
            std::fs::read(&live).expect("read entity after failed save"),
            original_bytes,
            "a failed save tore / half-wrote a pre-existing entity file"
        );
        // And the failure is honestly surfaced (not swallowed).
        assert!(result.is_err(), "save must surface the staging failure");
    }

    #[test]
    fn now_iso_is_rfc3339_shaped() {
        let ts = now_iso();
        assert_eq!(ts.len(), 20, "got {ts}");
        assert!(ts.ends_with('Z'));
        assert_eq!(&ts[4..5], "-");
        assert_eq!(&ts[10..11], "T");
    }
}
