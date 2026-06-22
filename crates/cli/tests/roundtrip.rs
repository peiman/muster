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
