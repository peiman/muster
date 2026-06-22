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
