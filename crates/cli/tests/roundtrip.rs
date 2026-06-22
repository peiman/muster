//! muster v3 integration tests — the declarative whole-store round-trip
//! (`state` / `apply`). The idiomatic TDD red→green lane mirroring the six
//! `acceptance/roundtrip.sh` criteria as precise Rust assertions, driven through
//! the public CLI in both surfaces (Manifesto #1, #7).

use assert_cmd::Command;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

fn data_dir(tmp: &TempDir) -> PathBuf {
    tmp.path().join(".muster")
}

fn muster(dir: &Path) -> Command {
    let mut c = Command::cargo_bin("muster").unwrap();
    c.env("MUSTER_DATA_DIR", dir).arg("--no-audit");
    c
}

/// Run a command in JSON mode, assert success, return the parsed `data`.
fn data(dir: &Path, args: &[&str]) -> Value {
    let out = muster(dir)
        .args(args)
        .args(["--output", "json"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "command {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: Value = serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
        panic!(
            "bad json for {args:?}: {e}\n{}",
            String::from_utf8_lossy(&out.stdout)
        )
    });
    assert_eq!(v["status"], "success");
    v["data"].clone()
}

/// Raw stdout bytes of a JSON-mode command (for byte-identity assertions).
fn raw_json(dir: &Path, args: &[&str]) -> Vec<u8> {
    let out = muster(dir)
        .args(args)
        .args(["--output", "json"])
        .output()
        .unwrap();
    assert!(out.status.success(), "command {args:?} failed");
    out.stdout
}

fn init(dir: &Path) {
    muster(dir).arg("init").assert().success();
}

/// Seed a store with a process and a ref-backed control whose `file_anchor`
/// points at an absolute path (cwd-independent resolution).
fn seed(dir: &Path, fixture: &Path) {
    init(dir);
    data(
        dir,
        &[
            "process",
            "add",
            "ship-roundtrip",
            "--name",
            "Ship the round-trip",
            "--owner",
            "peiman",
        ],
    );
    data(dir, &["process", "risk", "add", "ship-roundtrip", "drift"]);
    data(
        dir,
        &[
            "control",
            "add",
            "cov-bar",
            "--title",
            "coverage bar",
            "--ref-file",
            fixture.to_str().unwrap(),
            "--ref-anchor",
            "coverage.percent",
            "--expect",
            ">=80",
        ],
    );
    data(
        dir,
        &["process", "link-control", "ship-roundtrip", "cov-bar"],
    );
}

fn write_fixture(tmp: &TempDir) -> PathBuf {
    let fix = tmp.path().join("coverage.json");
    fs::write(&fix, "{\"coverage\": {\"percent\": 92}}\n").unwrap();
    fix
}

// ── SC-2 — full read, read-only ───────────────────────────────────────────────

#[test]
fn state_emits_every_entity_as_one_document() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    let fix = write_fixture(&tmp);
    seed(&d, &fix);

    let doc = data(&d, &["state"]);
    // The editable store — every category is present as an array.
    for arr in ["processes", "controls", "incidents", "nonconformities"] {
        assert!(doc[arr].is_array(), "{arr} must be a JSON array");
    }
    // The seeded entities are present (NOT a readiness verdict view).
    assert_eq!(doc["processes"][0]["id"], "ship-roundtrip");
    assert_eq!(doc["controls"][0]["id"], "cov-bar");
    // No verdict view leaked in.
    assert!(doc.get("verdict").is_none());
}

#[test]
fn state_emits_schema_version() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    let fix = write_fixture(&tmp);
    seed(&d, &fix);

    let doc = data(&d, &["state"]);
    assert_eq!(doc["schema_version"], 1, "state must emit schema_version");
}

#[test]
fn state_is_read_only_and_deterministic() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    let fix = write_fixture(&tmp);
    seed(&d, &fix);

    let a = raw_json(&d, &["state"]);
    let b = raw_json(&d, &["state"]);
    assert_eq!(a, b, "state must be byte-identical on repeat (read-only)");
}

#[test]
fn state_human_mode_mirrors_the_same_ids() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    let fix = write_fixture(&tmp);
    seed(&d, &fix);

    let out = muster(&d).arg("state").output().unwrap();
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("ship-roundtrip"),
        "human state missing process"
    );
    assert!(text.contains("cov-bar"), "human state missing control");
}

// ── SC-3 / SC-4 / SC-5 / SC-6 — apply ─────────────────────────────────────────

/// Capture `state --output json` to a file, returning its path + bytes.
fn capture_state(dir: &Path, tmp: &TempDir, name: &str) -> (PathBuf, Vec<u8>) {
    let bytes = raw_json(dir, &["state"]);
    let path = tmp.path().join(name);
    fs::write(&path, &bytes).unwrap();
    (path, bytes)
}

#[test]
fn apply_round_trip_is_a_fixpoint() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    let fix = write_fixture(&tmp);
    seed(&d, &fix);

    // Capture the literal envelope bytes `state` emits.
    let (manifest, s1) = capture_state(&d, &tmp, "s1.json");
    // Wipe the data dir, re-init, and apply the captured document.
    fs::remove_dir_all(&d).unwrap();
    init(&d);
    muster(&d)
        .args(["apply", manifest.to_str().unwrap()])
        .assert()
        .success();
    let s2 = raw_json(&d, &["state"]);
    assert_eq!(
        s1, s2,
        "round-trip is not a fixpoint: state != apply(state)"
    );
}

#[test]
fn apply_is_idempotent() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    let fix = write_fixture(&tmp);
    seed(&d, &fix);
    let (manifest, s1) = capture_state(&d, &tmp, "s1.json");

    // Apply twice over the existing store; both leave it byte-identical.
    for _ in 0..2 {
        muster(&d)
            .args(["apply", manifest.to_str().unwrap()])
            .assert()
            .success();
    }
    assert_eq!(raw_json(&d, &["state"]), s1, "apply is not idempotent");
}

#[test]
fn apply_dry_run_mutates_nothing_but_prints_verdict() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    let fix = write_fixture(&tmp);
    seed(&d, &fix);
    let (_m, s1) = capture_state(&d, &tmp, "s1.json");

    // A VALID-but-changed manifest (free-text only): would change the store.
    let changed = tmp.path().join("changed.json");
    let s1_str = String::from_utf8(s1.clone()).unwrap();
    fs::write(
        &changed,
        s1_str.replace("Ship the round-trip", "MUTATED-by-dry-run"),
    )
    .unwrap();

    let out = muster(&d)
        .args(["apply", "--dry-run", changed.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(out.status.success(), "dry-run must exit 0");
    let printed = String::from_utf8_lossy(&out.stdout).to_lowercase();
    assert!(
        printed.contains("ready") || printed.contains("gaps") || printed.contains("verdict"),
        "dry-run did not print a readiness verdict: {printed}"
    );
    // The store is untouched by the dry-run.
    assert_eq!(raw_json(&d, &["state"]), s1, "--dry-run mutated the store");
}

#[test]
fn apply_fails_closed_on_dangling_anchor_leaving_store_unchanged() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    let fix = write_fixture(&tmp);
    seed(&d, &fix);
    let (_m, s1) = capture_state(&d, &tmp, "s1.json");

    // Break ONLY the anchor string (keep the exact serde shape).
    let bad = tmp.path().join("bad.json");
    let s1_str = String::from_utf8(s1.clone()).unwrap();
    fs::write(
        &bad,
        s1_str.replace("coverage.percent", "coverage.DOES_NOT_EXIST"),
    )
    .unwrap();

    let out = muster(&d)
        .args(["apply", bad.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "apply of a dangling-anchor manifest must fail-closed"
    );
    // The error names the offending control and is honest about no mutation.
    let err = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    assert!(
        err.contains("cov-bar"),
        "error must name the control: {err}"
    );
    // The store was left exactly as it was (all-or-nothing).
    assert_eq!(
        raw_json(&d, &["state"]),
        s1,
        "a failed apply mutated the store (not all-or-nothing)"
    );
}

#[test]
fn apply_accepts_a_bare_document_without_the_envelope() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    let fix = write_fixture(&tmp);
    seed(&d, &fix);

    // Extract just the `data` (a bare StoreDocument) and apply it — accepted too.
    let envelope: Value = serde_json::from_slice(&raw_json(&d, &["state"])).unwrap();
    let bare = tmp.path().join("bare.json");
    fs::write(&bare, serde_json::to_vec(&envelope["data"]).unwrap()).unwrap();

    muster(&d)
        .args(["apply", bare.to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn apply_refuses_a_future_schema_version_leaving_store_unchanged() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    let fix = write_fixture(&tmp);
    seed(&d, &fix);
    let (_m, s1) = capture_state(&d, &tmp, "s1.json");

    // Bump the version past what the binary understands (string-replace, exact shape).
    let future = tmp.path().join("future.json");
    let s1_str = String::from_utf8(s1.clone()).unwrap();
    fs::write(
        &future,
        s1_str.replace("\"schema_version\": 1", "\"schema_version\": 999"),
    )
    .unwrap();

    let out = muster(&d)
        .args(["apply", future.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "apply of a future schema_version must fail-closed"
    );
    let err = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    assert!(
        err.contains("schema_version") || err.contains("version"),
        "error must mention the schema version: {err}"
    );
    assert_eq!(
        raw_json(&d, &["state"]),
        s1,
        "a refused future-version apply mutated the store"
    );
}

#[test]
fn apply_accepts_an_unversioned_manifest_as_v1() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    init(&d);

    // A legacy manifest that omits schema_version is accepted (defaults to v1).
    let legacy = tmp.path().join("legacy.json");
    fs::write(
        &legacy,
        r#"{"processes":[{"id":"p1","name":"P1","status":"proposed"}]}"#,
    )
    .unwrap();
    muster(&d)
        .args(["apply", legacy.to_str().unwrap()])
        .assert()
        .success();
    let doc = data(&d, &["state"]);
    assert_eq!(doc["processes"][0]["id"], "p1");
    assert_eq!(doc["schema_version"], 1);
}

#[test]
fn apply_refuses_an_unknown_field_leaving_store_unchanged() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    let fix = write_fixture(&tmp);
    seed(&d, &fix);
    let (_m, s1) = capture_state(&d, &tmp, "s1.json");

    // A bogus key on a control entity must be an honest error, never a silent drop.
    let unknown = tmp.path().join("unknown.json");
    fs::write(
        &unknown,
        r#"{"controls":[{"id":"c1","title":"C","applicable":true,"status":"not_started","evidence":[],"bogus_unknown_field":true}]}"#,
    )
    .unwrap();
    let out = muster(&d)
        .args(["apply", unknown.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "apply of an unknown-field manifest must fail-closed"
    );
    assert_eq!(
        raw_json(&d, &["state"]),
        s1,
        "a refused unknown-field apply mutated the store"
    );
}

#[test]
fn apply_refuses_a_duplicate_id_leaving_store_unchanged() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    let fix = write_fixture(&tmp);
    seed(&d, &fix);
    let (_m, s1) = capture_state(&d, &tmp, "s1.json");

    // Two controls share an id — last-write-wins is silent corruption, refuse it.
    let dup = tmp.path().join("dup.json");
    fs::write(
        &dup,
        r#"{"controls":[
          {"id":"c1","title":"A","applicable":true,"status":"not_started","evidence":[]},
          {"id":"c1","title":"B","applicable":true,"status":"not_started","evidence":[]}
        ]}"#,
    )
    .unwrap();
    let out = muster(&d)
        .args(["apply", dup.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!out.status.success(), "duplicate-id apply must fail-closed");
    let err = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    assert!(
        err.contains("c1"),
        "error must name the duplicate id: {err}"
    );
    assert_eq!(
        raw_json(&d, &["state"]),
        s1,
        "a refused duplicate-id apply mutated the store"
    );
}

#[test]
fn apply_refuses_an_invalid_slug_id_leaving_store_unchanged() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    init(&d);
    let s1 = raw_json(&d, &["state"]);

    let bad = tmp.path().join("badslug.json");
    fs::write(
        &bad,
        r#"{"controls":[{"id":"Bad Id","title":"C","applicable":true,"status":"not_started","evidence":[]}]}"#,
    )
    .unwrap();
    let out = muster(&d)
        .args(["apply", bad.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!out.status.success(), "invalid-slug apply must fail-closed");
    assert_eq!(
        raw_json(&d, &["state"]),
        s1,
        "a refused apply mutated the store"
    );
}

#[test]
fn apply_refuses_a_dangling_intra_document_ref_leaving_store_unchanged() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    init(&d);
    let s1 = raw_json(&d, &["state"]);

    // A process links a control present in neither the manifest nor the store.
    let bad = tmp.path().join("dangling.json");
    fs::write(
        &bad,
        r#"{"processes":[{"id":"p1","name":"P","status":"proposed","controls":["ghost"]}]}"#,
    )
    .unwrap();
    let out = muster(&d)
        .args(["apply", bad.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "dangling intra-doc ref apply must fail-closed"
    );
    let err = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    assert!(
        err.contains("ghost"),
        "error must name the dangling id: {err}"
    );
    assert_eq!(
        raw_json(&d, &["state"]),
        s1,
        "a refused apply mutated the store"
    );
}

#[test]
fn apply_leaves_no_temp_files_in_the_data_dir() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    let fix = write_fixture(&tmp);
    seed(&d, &fix);
    let (manifest, _s1) = capture_state(&d, &tmp, "s1.json");

    fs::remove_dir_all(&d).unwrap();
    init(&d);
    muster(&d)
        .args(["apply", manifest.to_str().unwrap()])
        .assert()
        .success();

    // The atomic write (temp-then-rename) must leave NO staging files behind.
    let mut leftovers = Vec::new();
    for sub in ["processes", "controls", "incidents", "nonconformities"] {
        let subdir = d.join(sub);
        if let Ok(entries) = fs::read_dir(&subdir) {
            for e in entries.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if name.ends_with(".tmp") {
                    leftovers.push(name);
                }
            }
        }
    }
    assert!(
        leftovers.is_empty(),
        "atomic save left stray temp files: {leftovers:?}"
    );
}

#[test]
fn apply_malformed_json_reports_the_not_valid_json_message() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    init(&d);
    let s1 = raw_json(&d, &["state"]);

    let bad = tmp.path().join("malformed.json");
    fs::write(&bad, "{ this is not json").unwrap();
    let out = muster(&d)
        .args(["apply", bad.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!out.status.success(), "malformed JSON must fail");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("not valid JSON"),
        "malformed JSON must report the not-valid-JSON message: {err}"
    );
    assert_eq!(
        raw_json(&d, &["state"]),
        s1,
        "a failed apply mutated the store"
    );
}

#[test]
fn apply_wrong_shape_reports_the_store_shape_message() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    init(&d);

    // Valid JSON, wrong shape (controls must be an array, not a number).
    let bad = tmp.path().join("wrongshape.json");
    fs::write(&bad, r#"{"controls": 5}"#).unwrap();
    let out = muster(&d)
        .args(["apply", bad.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!out.status.success(), "wrong-shape manifest must fail");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("does not match the store shape"),
        "wrong shape must report the store-shape message: {err}"
    );
}

#[test]
fn apply_missing_file_reports_the_read_error() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    init(&d);
    let missing = tmp.path().join("does-not-exist.json");
    let out = muster(&d)
        .args(["apply", missing.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!out.status.success(), "a missing manifest must fail");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("could not read manifest"),
        "missing file must report the read error: {err}"
    );
}

#[test]
fn apply_empty_object_manifest_is_accepted_and_changes_nothing() {
    // Pinned behavior: an empty `{}` manifest parses as a v1 document with all
    // categories empty; upsert prunes nothing (v3 is additive), so the store is
    // left exactly as it was.
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    let fix = write_fixture(&tmp);
    seed(&d, &fix);
    let s1 = raw_json(&d, &["state"]);

    let empty = tmp.path().join("empty.json");
    fs::write(&empty, "{}").unwrap();
    muster(&d)
        .args(["apply", empty.to_str().unwrap()])
        .assert()
        .success();
    assert_eq!(
        raw_json(&d, &["state"]),
        s1,
        "an empty manifest must leave the store unchanged (upsert prunes nothing)"
    );
}

/// Raw `state --output json` bytes with command-cache mode on (so a command-ref
/// seed carries a `resolved_ts` and the round-trip is exercised over a time-derived
/// field — a re-stamp/drop-timestamp regression a timestamp-free seed would miss).
fn raw_state_cc(dir: &Path) -> Vec<u8> {
    let out = muster(dir)
        .env("MUSTER_CMD_CACHE", "1")
        .args(["state", "--output", "json"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "state failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    out.stdout
}

/// Seed a command-ref control and resolve it so the store carries a `resolved_ts`.
fn seed_cmd_ref(dir: &Path) {
    init(dir);
    muster(dir)
        .env("MUSTER_CMD_CACHE", "1")
        .args([
            "control",
            "add",
            "cmd-ctrl",
            "--title",
            "cmd",
            "--ref-cmd",
            "true",
            "--ref-dir",
            ".",
        ])
        .assert()
        .success();
    muster(dir)
        .env("MUSTER_CMD_CACHE", "1")
        .args(["control", "resolve", "cmd-ctrl"])
        .assert()
        .success();
}

#[test]
fn apply_round_trip_preserves_a_resolved_timestamp() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    seed_cmd_ref(&d);

    let s1 = raw_state_cc(&d);
    let s1_str = String::from_utf8(s1.clone()).unwrap();
    assert!(
        s1_str.contains("resolved_ts"),
        "seed must carry a resolved_ts: {s1_str}"
    );
    let manifest = tmp.path().join("s1.json");
    fs::write(&manifest, &s1).unwrap();

    // Wipe, re-init, apply — the resolved_ts must survive byte-identically.
    fs::remove_dir_all(&d).unwrap();
    init(&d);
    muster(&d)
        .env("MUSTER_CMD_CACHE", "1")
        .args(["apply", manifest.to_str().unwrap()])
        .assert()
        .success();
    assert_eq!(
        raw_state_cc(&d),
        s1,
        "round-trip dropped or re-stamped the resolved timestamp"
    );
}

#[test]
fn apply_dry_run_preserves_a_resolved_timestamp() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    seed_cmd_ref(&d);
    let s1 = raw_state_cc(&d);

    // A changed-but-valid manifest (free-text title only): would change the store.
    let changed = tmp.path().join("changed.json");
    let s1_str = String::from_utf8(s1.clone()).unwrap();
    fs::write(
        &changed,
        s1_str.replace("\"title\": \"cmd\"", "\"title\": \"CMD-CHANGED\""),
    )
    .unwrap();

    muster(&d)
        .env("MUSTER_CMD_CACHE", "1")
        .args(["apply", "--dry-run", changed.to_str().unwrap()])
        .assert()
        .success();
    assert_eq!(
        raw_state_cc(&d),
        s1,
        "--dry-run mutated the store (timestamp re-stamped or cache written)"
    );
}

#[test]
fn apply_round_trip_preserves_multi_entity_ordering() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    init(&d);
    // Insert OUT of id order to prove ordering survives the disk round-trip.
    for (id, name) in [("zeta", "Zeta"), ("alpha", "Alpha")] {
        muster(&d)
            .args(["process", "add", id, "--name", name])
            .assert()
            .success();
    }
    for (id, title) in [("ctrl-b", "B"), ("ctrl-a", "A")] {
        muster(&d)
            .args(["control", "add", id, "--title", title])
            .assert()
            .success();
    }

    let (manifest, s1) = capture_state(&d, &tmp, "s1.json");
    fs::remove_dir_all(&d).unwrap();
    init(&d);
    muster(&d)
        .args(["apply", manifest.to_str().unwrap()])
        .assert()
        .success();
    assert_eq!(
        raw_json(&d, &["state"]),
        s1,
        "ordering did not survive the round-trip"
    );

    // And the output is id-sorted (deterministic).
    let doc = data(&d, &["state"]);
    let pids: Vec<&str> = doc["processes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["id"].as_str().unwrap())
        .collect();
    let cids: Vec<&str> = doc["controls"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert_eq!(pids, vec!["alpha", "zeta"], "processes must be id-sorted");
    assert_eq!(cids, vec!["ctrl-a", "ctrl-b"], "controls must be id-sorted");
}

#[test]
fn apply_fail_closed_names_only_the_broken_control() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    init(&d);
    // A fixture with two distinct anchors, so each control breaks independently.
    let fix = tmp.path().join("cov.json");
    fs::write(&fix, "{\"coverage\": {\"percent\": 92, \"other\": 50}}\n").unwrap();
    muster(&d)
        .args([
            "control",
            "add",
            "cov-a",
            "--title",
            "A",
            "--ref-file",
            fix.to_str().unwrap(),
            "--ref-anchor",
            "coverage.percent",
            "--expect",
            ">=80",
        ])
        .assert()
        .success();
    muster(&d)
        .args([
            "control",
            "add",
            "cov-b",
            "--title",
            "B",
            "--ref-file",
            fix.to_str().unwrap(),
            "--ref-anchor",
            "coverage.other",
            "--expect",
            ">=10",
        ])
        .assert()
        .success();
    let (_m, s1) = capture_state(&d, &tmp, "s1.json");

    // Break ONLY cov-b's anchor (cov-a stays healthy).
    let bad = tmp.path().join("bad.json");
    let s1_str = String::from_utf8(s1.clone()).unwrap();
    fs::write(&bad, s1_str.replace("coverage.other", "coverage.MISSING")).unwrap();

    let out = muster(&d)
        .args(["apply", bad.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "apply with a broken anchor must fail-closed"
    );
    let err = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    assert!(
        err.contains("cov-b"),
        "error must name the broken control: {err}"
    );
    assert!(
        !err.contains("cov-a"),
        "error must NOT name the healthy control: {err}"
    );
    assert_eq!(
        raw_json(&d, &["state"]),
        s1,
        "a refused apply mutated the store"
    );
}

// ── SC-7 — discoverability (explain + catalog list both verbs) ─────────────────

#[test]
fn catalog_json_lists_state_and_apply() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    init(&d);
    let cat = data(&d, &["catalog"]);
    let names: Vec<&str> = cat["commands"]
        .as_array()
        .expect("commands is an array")
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"state"), "catalog omits 'state': {names:?}");
    assert!(names.contains(&"apply"), "catalog omits 'apply': {names:?}");
}

#[test]
fn explain_lists_state_and_apply() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    init(&d);
    let out = muster(&d).arg("explain").output().unwrap();
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("muster state"), "explain omits 'state'");
    assert!(text.contains("muster apply"), "explain omits 'apply'");
}
