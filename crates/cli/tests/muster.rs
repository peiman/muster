//! muster integration tests (assert_cmd) — each in its own isolated
//! `MUSTER_DATA_DIR`. These are the validator's spine: they drive the full
//! management-system flow through the public CLI in both surfaces and assert
//! truthful results (Manifesto #1).

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use serde_json::Value;
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

fn init(dir: &Path) {
    muster(dir).arg("init").assert().success();
}

/// SC-2…SC-8 — the DoD spine, driven end-to-end through `--output json`.
#[test]
fn dod_full_spine() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    init(&d);

    // 1. add process — defaults to proposed (SC-2)
    let p = data(
        &d,
        &[
            "process",
            "add",
            "incident-mgmt",
            "--name",
            "Incident Management",
            "--owner",
            "ciso",
        ],
    );
    assert_eq!(p["id"], "incident-mgmt");
    assert_eq!(p["name"], "Incident Management");
    assert_eq!(p["owner"], "ciso");
    assert_eq!(p["status"], "proposed");
    for arr in [
        "steps",
        "controls",
        "risks",
        "metrics",
        "checks",
        "revisions",
        "evidence",
    ] {
        assert!(p[arr].is_array(), "{arr} must be a JSON array");
    }

    // 2. recursive graph + tree (SC-3)
    data(
        &d,
        &["process", "add", "containment", "--name", "Containment"],
    );
    data(
        &d,
        &[
            "process",
            "step",
            "add",
            "incident-mgmt",
            "--description",
            "Contain",
            "--process-ref",
            "containment",
        ],
    );
    let tree = data(&d, &["process", "show", "incident-mgmt", "--tree"]);
    let step = &tree["steps"][0];
    assert_eq!(step["process_ref"], "containment");
    assert_eq!(step["sub"]["kind"], "process");
    assert_eq!(step["sub"]["id"], "containment");

    // 3. controls + linking + evidence (SC-4)
    data(
        &d,
        &[
            "control",
            "add",
            "a5-24",
            "--title",
            "Incident planning",
            "--clause-ref",
            "ISO 27001 A.5.24",
        ],
    );
    data(&d, &["process", "link-control", "incident-mgmt", "a5-24"]);
    data(&d, &["control", "set-status", "a5-24", "implemented"]);
    data(
        &d,
        &[
            "control",
            "attach-evidence",
            "a5-24",
            // a verifying artifact: note-only is honor-level and no longer covers (v3.1).
            "file",
            "runbook.md",
        ],
    );
    let c = data(&d, &["control", "show", "a5-24"]);
    assert_eq!(c["status"], "implemented");
    assert_eq!(c["clause_ref"], "ISO 27001 A.5.24");
    assert!(!c["evidence"].as_array().unwrap().is_empty());
    let p = data(&d, &["process", "show", "incident-mgmt"]);
    assert!(
        p["controls"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "a5-24")
    );

    // 4. conformance check ingest, the #9 seam (SC-5)
    let p = data(
        &d,
        &[
            "process",
            "check",
            "add",
            "incident-mgmt",
            "--description",
            "runbook exists in CI",
            "--enforcement",
            "ci",
        ],
    );
    let check_id = p["checks"][0]["id"].as_str().unwrap().to_string();
    assert_eq!(p["checks"][0]["last_result"], "unknown");
    assert_eq!(p["checks"][0]["enforcement"], "ci");
    let p = data(
        &d,
        &["process", "check", "incident-mgmt", &check_id, "--pass"],
    );
    assert_eq!(p["checks"][0]["last_result"], "pass");
    assert!(!p["checks"][0]["last_run_ts"].as_str().unwrap().is_empty());

    // 5. incident C2 (SC-6)
    let i = data(
        &d,
        &[
            "incident",
            "report",
            "inc-1",
            "--title",
            "Outage",
            "--severity",
            "high",
            "--process",
            "incident-mgmt",
        ],
    );
    assert_eq!(i["status"], "open");
    assert_eq!(i["severity"], "high");
    assert_eq!(i["process_ref"], "incident-mgmt");
    let i = data(&d, &["incident", "log", "inc-1", "contained"]);
    assert_eq!(i["log"].as_array().unwrap().len(), 1);
    assert!(i["log"][0]["ts"].is_string() && i["log"][0]["note"] == "contained");

    // 6. nonconformity + the #10 feedback cycle (SC-7)
    let nc = data(
        &d,
        &[
            "nonconformity",
            "raise",
            "nc-1",
            "--from-incident",
            "inc-1",
            "--description",
            "detection too slow",
        ],
    );
    assert_eq!(nc["source"], "incident");
    assert_eq!(nc["process_ref"], "incident-mgmt");
    assert_eq!(nc["status"], "open");
    let p = data(
        &d,
        &[
            "process",
            "revise",
            "incident-mgmt",
            "tightened detection step",
            "--because",
            "nc-1",
        ],
    );
    let rev = &p["revisions"][0];
    assert_eq!(rev["summary"], "tightened detection step");
    assert_eq!(rev["because"], "nc-1");
    assert!(rev["ts"].is_string());
    let nc = data(
        &d,
        &[
            "nonconformity",
            "resolve",
            "nc-1",
            "--corrective-action",
            "added automated alert",
        ],
    );
    assert_eq!(nc["status"], "closed");
    assert_eq!(nc["corrective_action"], "added automated alert");
    data(&d, &["incident", "close", "inc-1"]);

    // 7. readiness is an honest truth-meter (SC-8)
    data(&d, &["process", "set-status", "incident-mgmt", "active"]);
    let r = data(&d, &["readiness"]);
    for key in [
        "verdict",
        "control_coverage",
        "proven",
        "asserted",
        "refuting_signals",
        "enforcement",
        "gap_findings",
        "cycles",
    ] {
        assert!(r.get(key).is_some(), "readiness missing key {key}");
    }
    let cov = &r["control_coverage"];
    for key in ["applicable", "implemented_with_evidence", "percent", "gaps"] {
        assert!(cov.get(key).is_some(), "control_coverage missing {key}");
    }
    // resolved nc-1 / closed inc-1 leave no refuting signal for incident-mgmt
    assert!(
        !r["refuting_signals"].as_array().unwrap().iter().any(|s| {
            let src = s["source"].as_str().unwrap_or("");
            src.contains("nc-1") || src.contains("inc-1")
        }),
        "resolved/closed signals must not refute: {:?}",
        r["refuting_signals"]
    );
    // strongest enforcement for incident-mgmt == ci
    let enf = r["enforcement"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["process_id"] == "incident-mgmt")
        .unwrap();
    assert_eq!(enf["strongest"], "ci");
    // coverage math: 1 of 1 applicable
    assert_eq!(cov["applicable"], 1);
    assert_eq!(cov["implemented_with_evidence"], 1);
    assert_eq!(cov["percent"], 100.0);
    // verdict honest: never READY while gaps exist
    let n_gaps = r["gap_findings"].as_array().unwrap().len();
    if n_gaps == 0 && r["refuting_signals"].as_array().unwrap().is_empty() {
        assert_eq!(r["verdict"], "READY");
    } else {
        assert_eq!(r["verdict"], format!("GAPS: {n_gaps}"));
        assert_ne!(r["verdict"], "READY");
    }
}

/// SC-9 — mutating state moves the numbers in at least two distinct fields.
#[test]
fn readiness_moves() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    init(&d);
    data(&d, &["process", "add", "p1", "--name", "P1"]);
    data(&d, &["control", "add", "c1", "--title", "C1"]);
    data(&d, &["control", "set-status", "c1", "implemented"]);
    // a verifying artifact: note-only is honor-level and no longer covers (v3.1).
    data(
        &d,
        &["control", "attach-evidence", "c1", "file", "evidence.txt"],
    );

    let before = data(&d, &["readiness"]);
    assert_eq!(before["control_coverage"]["percent"], 100.0);

    // Add a second applicable control with no evidence → coverage drops + gap grows.
    data(&d, &["control", "add", "c2", "--title", "C2"]);
    let after = data(&d, &["readiness"]);
    assert_eq!(after["control_coverage"]["percent"], 50.0);
    assert!(
        after["control_coverage"]["gaps"].as_array().unwrap().len()
            > before["control_coverage"]["gaps"].as_array().unwrap().len()
    );
    // two distinct fields moved: percent AND gaps
    assert_ne!(
        before["control_coverage"]["percent"],
        after["control_coverage"]["percent"]
    );
}

/// SC-10 — a cycle is detected, both readiness and tree terminate.
#[test]
fn cycle_terminates() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    init(&d);
    data(&d, &["process", "add", "a", "--name", "A"]);
    data(&d, &["process", "add", "b", "--name", "B"]);
    data(
        &d,
        &[
            "process",
            "step",
            "add",
            "a",
            "--description",
            "to b",
            "--process-ref",
            "b",
        ],
    );
    data(
        &d,
        &[
            "process",
            "step",
            "add",
            "b",
            "--description",
            "to a",
            "--process-ref",
            "a",
        ],
    );

    let r = data(&d, &["readiness"]);
    let cycles = r["cycles"].as_array().unwrap();
    assert!(!cycles.is_empty(), "cycle must be reported");
    assert!(cycles[0].as_array().unwrap().len() >= 2);

    // tree must also terminate (cycle marker, not infinite recursion)
    let tree = data(&d, &["process", "show", "a", "--tree"]);
    let b = &tree["steps"][0]["sub"];
    assert_eq!(b["kind"], "process");
    assert_eq!(b["steps"][0]["sub"]["kind"], "cycle");
}

/// SC-11 — every human-visible fact is present as a JSON field; no markdown in JSON.
#[test]
fn dual_surface_parity() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    init(&d);
    data(
        &d,
        &[
            "process", "add", "p1", "--name", "Payments", "--owner", "cto",
        ],
    );

    // human surface
    let human = muster(&d).args(["process", "show", "p1"]).output().unwrap();
    let text = String::from_utf8(human.stdout).unwrap();
    // JSON surface
    let json = data(&d, &["process", "show", "p1"]);

    // facts in the human text exist as JSON fields
    assert!(text.contains("p1") && json["id"] == "p1");
    assert!(text.contains("Payments") && json["name"] == "Payments");
    assert!(text.contains("cto") && json["owner"] == "cto");
    assert!(text.contains("proposed") && json["status"] == "proposed");

    // JSON has no embedded markdown / pre-rendered tables
    let raw = serde_json::to_string(&json).unwrap();
    assert!(
        !raw.contains("Next:"),
        "guidance must not leak into JSON data"
    );
    assert!(!raw.contains('|'), "no pre-rendered tables in JSON");
    assert!(!raw.contains("```"), "no markdown fences in JSON");
}

/// SC-12 — honest signals: errors exit non-zero and name the offender + the fix.
#[test]
fn honest_errors() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);

    // command before init
    muster(&d)
        .args(["process", "list"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("muster init"));

    init(&d);

    // unknown id names the offender and the corrective command
    muster(&d)
        .args(["process", "show", "ghost"])
        .assert()
        .failure()
        .stderr(
            predicates::str::contains("ghost").and(predicates::str::contains("muster process add")),
        );

    // bad enum names the allowed values
    muster(&d)
        .args(["process", "add", "p1", "--name", "P1"])
        .assert()
        .success();
    muster(&d)
        .args([
            "process",
            "check",
            "add",
            "p1",
            "--description",
            "d",
            "--enforcement",
            "bogus",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("compile_time"));

    // invalid slug rejected
    muster(&d)
        .args(["process", "add", "Bad Id", "--name", "x"])
        .assert()
        .failure();

    // explain works without a manual
    muster(&d)
        .arg("explain")
        .assert()
        .success()
        .stdout(predicates::str::contains("muster readiness"));
}

/// SC-11 (determinism) — unchanged state yields byte-identical JSON across runs.
#[test]
fn determinism() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    init(&d);
    // insert out of id order; output must be id-sorted + stable.
    data(&d, &["process", "add", "zeta", "--name", "Z"]);
    data(&d, &["process", "add", "alpha", "--name", "A"]);
    data(&d, &["control", "add", "c2", "--title", "C2"]);
    data(&d, &["control", "add", "c1", "--title", "C1"]);

    let run = || {
        let out = muster(&d)
            .args(["process", "list", "--output", "json"])
            .output()
            .unwrap();
        out.stdout
    };
    assert_eq!(run(), run(), "process list JSON must be byte-stable");

    // id-sorted: alpha before zeta
    let v: Value = serde_json::from_slice(&run()).unwrap();
    let ids: Vec<&str> = v["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec!["alpha", "zeta"]);

    let readiness_stable = || {
        muster(&d)
            .args(["readiness", "--output", "json"])
            .output()
            .unwrap()
            .stdout
    };
    assert_eq!(readiness_stable(), readiness_stable());
}

// ─────────────────────────────────────────────────────────────────────────────
// v1 glue (SC-3…SC-11) — reference-backed controls/checks, derived on read.
// ─────────────────────────────────────────────────────────────────────────────

/// Write `body` to `name` under `tmp`, returning the absolute path as a String.
fn write_src(tmp: &TempDir, name: &str, body: &str) -> String {
    let p = tmp.path().join(name);
    std::fs::write(&p, body).unwrap();
    p.to_string_lossy().into_owned()
}

/// Run in JSON mode WITHOUT asserting success; return (success, parsed envelope).
fn run_json(dir: &Path, args: &[&str]) -> (bool, Value) {
    let out = muster(dir)
        .args(args)
        .args(["--output", "json"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|_| serde_json::json!({"status":"error"}));
    (out.status.success(), v)
}

#[test]
fn sc3_title_is_a_resolved_projection() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    init(&d);
    let src = write_src(&tmp, "src.toml", "[requirements.r1]\ntitle = \"Alpha\"\n");
    data(
        &d,
        &[
            "control",
            "add",
            "c1",
            "--title",
            "placeholder",
            "--ref-file",
            &src,
            "--ref-anchor",
            "requirements.r1.title",
        ],
    );
    let c = data(&d, &["control", "show", "c1"]);
    assert_eq!(c["title"], "Alpha", "title must derive from source");
    assert_eq!(c["resolution"]["resolution_state"], "derived");
    assert_eq!(c["fallback_title"], "placeholder");

    // Edit the source → muster reflects it on the next read (not stale-forever).
    std::fs::write(&src, "[requirements.r1]\ntitle = \"Beta\"\n").unwrap();
    let c2 = data(&d, &["control", "show", "c1"]);
    assert_eq!(
        c2["title"], "Beta",
        "edit must reflect with no muster mutation"
    );
}

#[test]
fn expect_turns_a_numeric_metric_into_an_honest_live_verdict() {
    // The numeric-threshold evolution (dogfood 2026-06-19): a bare number is
    // Unknown, but `--expect ">=80"` makes it an honest Pass/Fail — and it stays
    // LIVE (editing the source flips the verdict on read, no muster mutation).
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    init(&d);
    let src = write_src(&tmp, "coverage.json", "{\"percent\": 85}\n");
    data(
        &d,
        &[
            "control",
            "add",
            "cov",
            "--ref-file",
            &src,
            "--ref-anchor",
            "percent",
            "--expect",
            ">=80",
        ],
    );
    let c = data(&d, &["control", "show", "cov"]);
    assert_eq!(
        c["resolution"]["outcome"], "pass",
        "85 ≥ 80 → pass, got {c}"
    );

    // Drop coverage below the bar → Fail on the next read (live glue, no mutation).
    std::fs::write(&src, "{\"percent\": 50}\n").unwrap();
    let c2 = data(&d, &["control", "show", "cov"]);
    assert_eq!(
        c2["resolution"]["outcome"], "fail",
        "50 < 80 → fail on read, got {c2}"
    );
    let r = data(&d, &["readiness"]);
    assert_ne!(r["verdict"], "READY", "below the bar must not read READY");

    // A malformed --expect is an honest error, never a silent pass.
    let (ok, _v) = run_json(
        &d,
        &[
            "control",
            "add",
            "bad",
            "--ref-file",
            &src,
            "--ref-anchor",
            "percent",
            "--expect",
            "80",
        ],
    );
    assert!(!ok, "a malformed --expect must fail, not silently succeed");
}

#[test]
fn title_is_optional_when_a_ref_is_supplied() {
    // Tidy (SPEC P0 — "title as a resolved projection"): when a ref backs the
    // control, the title is DERIVED, so `--title` is an optional fallback. The
    // stored fallback defaults to the control id (legible when unresolved).
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    init(&d);
    let src = write_src(&tmp, "src.toml", "[requirements.r1]\ntitle = \"Alpha\"\n");
    // No --title, but a ref is supplied → succeeds, title derives from source.
    data(
        &d,
        &[
            "control",
            "add",
            "c1",
            "--ref-file",
            &src,
            "--ref-anchor",
            "requirements.r1.title",
        ],
    );
    let c = data(&d, &["control", "show", "c1"]);
    assert_eq!(c["title"], "Alpha", "title must derive from source");
    assert_eq!(c["resolution"]["resolution_state"], "derived");
    assert_eq!(
        c["fallback_title"], "c1",
        "absent --title falls back to the control id"
    );

    // No --title and NO ref → asserted controls still need a human label.
    let (ok, env) = run_json(&d, &["control", "add", "c2"]);
    assert!(!ok, "an asserted (no-ref) control must require --title");
    assert!(
        env["error"].as_str().unwrap_or("").contains("--title"),
        "error must name the missing --title flag: {env}"
    );
}

#[test]
fn sc4_and_sc5_checks_derive_and_cannot_be_forged() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    init(&d);
    let src = write_src(&tmp, "s.toml", "[r.r1]\nstatus = \"unmet\"\n");
    data(&d, &["process", "add", "p1", "--name", "P1"]);
    data(
        &d,
        &[
            "process",
            "check",
            "add",
            "p1",
            "--description",
            "arch",
            "--enforcement",
            "ci",
            "--ref-file",
            &src,
            "--ref-anchor",
            "r.r1.status",
        ],
    );
    // SC-4: derived fail.
    let p = data(&d, &["process", "show", "p1"]);
    assert_eq!(p["checks"][0]["last_result"], "fail");
    // SC-5: hand-setting pass on a ref-backed check is rejected.
    let (ok, env) = run_json(&d, &["process", "check", "p1", "check-1", "--pass"]);
    assert!(!ok, "forging a pass on a ref-backed check must fail");
    assert!(
        env["error"]
            .as_str()
            .unwrap_or("")
            .contains("reference-backed"),
        "error must name the ref as authority: {env}"
    );
    // Flip source to met → derived pass after a resolve.
    std::fs::write(&src, "[r.r1]\nstatus = \"met\"\n").unwrap();
    let p2 = data(&d, &["process", "show", "p1"]);
    // file_anchor re-resolves live on read.
    assert_eq!(p2["checks"][0]["last_result"], "pass");
}

#[test]
fn sc6_dangling_ref_is_refused_at_store_time_then_unresolved_is_a_gap() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    init(&d);
    // v2 (SC-5, Manifesto #9 fail-closed): a dangling file_anchor is REFUSED at
    // creation, naming the fix — no control is persisted.
    let (ok, env) = run_json(
        &d,
        &[
            "control",
            "add",
            "c1",
            "--title",
            "x",
            "--ref-file",
            "/no/such/file.toml",
            "--ref-anchor",
            "a.b",
        ],
    );
    assert!(!ok, "adding a dangling file_anchor must be refused");
    let err = env["error"].as_str().unwrap_or("");
    assert!(
        err.contains("/no/such/file.toml") && err.contains("a.b"),
        "refusal must name the path + anchor: {env}"
    );
    // And it is not persisted (the doctor surface confirms it is absent).
    let list = data(&d, &["control", "list"]);
    assert!(
        list.as_array().unwrap().is_empty(),
        "refused control must not be persisted: {list}"
    );

    // A ref that resolves at creation but whose source later disappears surfaces
    // as an honest ref_unresolved gap in readiness (the rot the doctor catches).
    let src = write_src(&tmp, "src.toml", "[r.r1]\nstatus = \"met\"\n");
    data(
        &d,
        &[
            "control",
            "add",
            "c2",
            "--title",
            "x",
            "--ref-file",
            &src,
            "--ref-anchor",
            "r.r1.status",
        ],
    );
    std::fs::remove_file(&src).unwrap();
    let c = data(&d, &["control", "show", "c2"]);
    assert_eq!(c["resolution"]["resolution_state"], "unresolved");
    let r = data(&d, &["readiness"]);
    assert!(r["verdict"].as_str().unwrap().starts_with("GAPS"));
    assert!(
        r["gap_findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|g| g["kind"] == "ref_unresolved"),
        "missing ref must surface a ref_unresolved gap: {}",
        r["gap_findings"]
    );
}

#[test]
fn sc7_command_ref_goes_stale_at_freshness_zero_in_cache_mode() {
    // Re-homed to opt-in cache mode (MUSTER_CMD_CACHE=1): only there is a command
    // verdict served from a cache that can go Stale. In the honest default a
    // command ref re-resolves live (see sc1_drift_window_closed).
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    init(&d);
    let dir = tmp.path().to_string_lossy().into_owned();
    // Add WITH cache on so a resolution is persisted to serve later.
    muster(&d)
        .env("MUSTER_CMD_CACHE", "1")
        .args([
            "control",
            "add",
            "c1",
            "--title",
            "x",
            "--ref-cmd",
            "true",
            "--ref-dir",
            &dir,
            "--output",
            "json",
        ])
        .assert()
        .success();
    // With freshness 0 + cache on, the served command cache projects to stale.
    let out = muster(&d)
        .env("MUSTER_CMD_CACHE", "1")
        .env("MUSTER_FRESHNESS_SECS", "0")
        .args(["control", "show", "c1", "--output", "json"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["data"]["resolution"]["resolution_state"], "stale");
    assert_eq!(v["data"]["resolution"]["served_from_cache"], true);
}

#[test]
fn sc8_readiness_splits_derived_vs_asserted() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    init(&d);
    let src = write_src(&tmp, "s.toml", "[r.r1]\nstatus = \"met\"\n");
    data(
        &d,
        &[
            "control",
            "add",
            "derived-ok",
            "--title",
            "x",
            "--ref-file",
            &src,
            "--ref-anchor",
            "r.r1.status",
        ],
    );
    data(&d, &["control", "add", "asserted-one", "--title", "y"]);
    let r = data(&d, &["readiness"]);
    let derived = r["controls"]["derived"].as_array().unwrap();
    let asserted = r["controls"]["asserted"].as_array().unwrap();
    assert!(
        derived.iter().any(|x| x == "derived-ok"),
        "derived list: {derived:?}"
    );
    assert!(
        asserted.iter().any(|x| x == "asserted-one"),
        "asserted list: {asserted:?}"
    );
}

#[test]
fn sc9_reference_import_ties_controls_to_source() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    init(&d);
    let manifest = write_src(
        &tmp,
        "reqs.toml",
        "[requirements.R1]\ntitle = \"One\"\n[requirements.R2]\ntitle = \"Two\"\n[requirements.R3]\ntitle = \"Three\"\n",
    );
    let imp = data(&d, &["control", "import", &manifest]);
    assert_eq!(imp["created"].as_array().unwrap().len(), 3);
    let c = data(&d, &["control", "show", "r1"]);
    assert_eq!(c["title"], "One");
    assert_eq!(c["ref"]["path"], manifest);
    // Edit the manifest → imported control's shown title changes (reference, not copy).
    std::fs::write(
        &manifest,
        "[requirements.R1]\ntitle = \"One-Edited\"\n[requirements.R2]\ntitle = \"Two\"\n[requirements.R3]\ntitle = \"Three\"\n",
    )
    .unwrap();
    let c2 = data(&d, &["control", "show", "r1"]);
    assert_eq!(c2["title"], "One-Edited");
}

#[test]
fn sc10_nm_one_failing_implementation_blocks_green() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    init(&d);
    let met = write_src(&tmp, "rust.toml", "[s]\nv = \"met\"\n");
    let unmet = write_src(&tmp, "go.toml", "[s]\nv = \"unmet\"\n");
    data(&d, &["control", "add", "c1", "--title", "Cross-impl"]);
    data(
        &d,
        &[
            "control",
            "add-implementation",
            "c1",
            "--impl-id",
            "rust",
            "--ref-file",
            &met,
            "--ref-anchor",
            "s.v",
        ],
    );
    data(
        &d,
        &[
            "control",
            "add-implementation",
            "c1",
            "--impl-id",
            "go",
            "--ref-file",
            &unmet,
            "--ref-anchor",
            "s.v",
        ],
    );
    let c = data(&d, &["control", "show", "c1"]);
    let impls = c["implementations"].as_array().unwrap();
    assert_eq!(impls.len(), 2);
    assert_ne!(
        c["status"], "implemented",
        "one failing impl must block green"
    );
}

#[test]
fn sc11_ckeletin_acceptance_refuses_green_over_red() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    init(&d);
    // Copy the real ckeletin manifest into a tempdir, flip ARCH-001 to unmet.
    let real = "/Users/peiman/dev/ckeletin-rust/conformance-mapping.toml";
    let body = match std::fs::read_to_string(real) {
        Ok(b) => b,
        Err(_) => return, // offline-safe: skip if the source isn't present.
    };
    let flipped = body.replacen(
        "[requirements.CKSPEC-ARCH-001]\ntitle = \"Four-layer architecture\"\nstatus = \"met\"",
        "[requirements.CKSPEC-ARCH-001]\ntitle = \"Four-layer architecture\"\nstatus = \"unmet\"",
        1,
    );
    assert!(flipped.contains("status = \"unmet\""), "flip must apply");
    let copy = write_src(&tmp, "conformance-mapping.toml", &flipped);
    data(
        &d,
        &[
            "control",
            "add",
            "arch",
            "--title",
            "placeholder",
            "--ref-file",
            &copy,
            "--ref-anchor",
            "requirements.CKSPEC-ARCH-001.title",
        ],
    );
    data(&d, &["process", "add", "p1", "--name", "P1"]);
    data(
        &d,
        &[
            "process",
            "check",
            "add",
            "p1",
            "--description",
            "arch",
            "--enforcement",
            "compile_time",
            "--ref-file",
            &copy,
            "--ref-anchor",
            "requirements.CKSPEC-ARCH-001.status",
        ],
    );
    // Title derives from the real file.
    let c = data(&d, &["control", "show", "arch"]);
    assert_eq!(c["title"], "Four-layer architecture");
    // Check derives fail from the flipped status — muster refuses to show green.
    let p = data(&d, &["process", "show", "p1"]);
    assert_eq!(p["checks"][0]["last_result"], "fail");
}

#[test]
fn sc13_explain_and_catalog_cover_the_glue_commands() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    init(&d);

    // explain maps the new intents to the new commands.
    let ex = data(&d, &["explain"]);
    let intents = ex["intents"].as_array().unwrap();
    let cmds: Vec<&str> = intents
        .iter()
        .map(|i| i["command"].as_str().unwrap())
        .collect();
    assert!(
        cmds.iter().any(|c| c.contains("--ref-file")),
        "explain must teach --ref-file"
    );
    assert!(
        cmds.iter().any(|c| c.contains("control import")),
        "explain must teach import"
    );
    assert!(
        cmds.iter().any(|c| c.contains("add-implementation")),
        "explain must teach N:M"
    );

    // catalog enumerates the new subcommands + flags (derived from the clap tree).
    let cat = data(&d, &["catalog"]);
    let control = cat["commands"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "control")
        .unwrap();
    let sub_names: Vec<&str> = control["commands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    for expected in ["resolve", "add-implementation", "import"] {
        assert!(
            sub_names.contains(&expected),
            "catalog missing control {expected}: {sub_names:?}"
        );
    }
    // The add subcommand exposes the ref flags structurally.
    let add = control["commands"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "add")
        .unwrap();
    let flags: Vec<&str> = add["flags"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["long"].as_str().unwrap())
        .collect();
    assert!(
        flags.contains(&"ref-file"),
        "catalog control add missing --ref-file: {flags:?}"
    );
}

// ── v2: honest glue — no stale green, the safe path is the default ────────────

/// SC-1 (the headline): DRIFT-WINDOW-CLOSED. A `--ref-cmd` control derives green
/// while the guarded file exists; delete the file and re-read WITHOUT any
/// intervening resolve — the control is no longer green (it re-resolved live),
/// outcome is fail, and the resolved age is surfaced (Manifesto #9).
#[test]
fn sc1_drift_window_closed() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    init(&d);
    let dir = tmp.path().to_string_lossy().into_owned();
    let guard = tmp.path().join("guard.txt");
    std::fs::write(&guard, "x").unwrap();
    let cmd = format!("test -f {}", guard.display());

    data(
        &d,
        &[
            "control",
            "add",
            "c-drift",
            "--title",
            "x",
            "--ref-cmd",
            &cmd,
            "--ref-dir",
            &dir,
        ],
    );
    let c = data(&d, &["control", "show", "c-drift"]);
    assert_eq!(c["status"], "implemented", "green while guard present");
    assert_eq!(c["resolution"]["resolution_state"], "derived");
    assert_eq!(c["resolution"]["outcome"], "pass");
    assert_eq!(
        c["resolution"]["resolved_age_secs"], 0,
        "live read is age 0"
    );
    assert_eq!(c["resolution"]["served_from_cache"], false);

    // Delete the guarded file; re-read with NO resolve call.
    std::fs::remove_file(&guard).unwrap();
    let c2 = data(&d, &["control", "show", "c-drift"]);
    assert_ne!(
        c2["status"], "implemented",
        "control must NOT be green after the guard is deleted: {c2}"
    );
    assert_eq!(
        c2["resolution"]["outcome"], "fail",
        "re-resolved live to fail"
    );
    assert!(
        c2["resolution"]["resolved_age_secs"].is_number(),
        "resolved age must still be surfaced: {c2}"
    );
}

/// SC-2: resolved age + served_from_cache present in JSON AND rendered in human.
#[test]
fn sc2_resolved_age_surfaced_both_surfaces() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    init(&d);
    let src = write_src(&tmp, "s.toml", "[r.r1]\nstatus = \"met\"\n");
    data(
        &d,
        &[
            "control",
            "add",
            "c1",
            "--title",
            "x",
            "--ref-file",
            &src,
            "--ref-anchor",
            "r.r1.status",
        ],
    );
    let c = data(&d, &["control", "show", "c1"]);
    assert_eq!(c["resolution"]["resolved_age_secs"], 0);
    assert_eq!(c["resolution"]["served_from_cache"], false);
    // Human surface mirrors it.
    let out = muster(&d)
        .args(["control", "show", "c1", "--output", "text"])
        .output()
        .unwrap();
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("age=0s"), "human must show resolved age: {s}");
    assert!(s.contains("live"), "human must show cache status: {s}");
}

/// SC-3: the safe path is reachable + steered. `--ref-report` makes a zero-drift
/// file_anchor in one flag; `--ref-cmd` add emits a steering_notice (both
/// surfaces); readiness exposes a per-control ref-kind drift profile.
#[test]
fn sc3_safe_path_reachable_and_steered() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    init(&d);
    let dir = tmp.path().to_string_lossy().into_owned();
    let report = write_src(
        &tmp,
        "report.json",
        r#"{"requirements":{"X":{"status":"met"}}}"#,
    );

    // --ref-report: one flag, derived file_anchor.
    let rpt = data(
        &d,
        &[
            "control",
            "add",
            "c-rpt",
            "--ref-report",
            &report,
            "requirements.X.status",
        ],
    );
    assert_eq!(rpt["ref"]["kind"], "file_anchor");
    let c = data(&d, &["control", "show", "c-rpt"]);
    assert_eq!(c["resolution"]["resolution_state"], "derived");

    // --ref-cmd: steering notice in JSON.
    let add = data(
        &d,
        &[
            "control",
            "add",
            "c-cmd",
            "--title",
            "x",
            "--ref-cmd",
            "true",
            "--ref-dir",
            &dir,
        ],
    );
    assert!(
        add["steering_notice"]
            .as_str()
            .unwrap_or("")
            .contains("--ref-report"),
        "steering notice must recommend the report path: {add}"
    );
    // ... and in the human surface.
    let out = muster(&d)
        .args([
            "control",
            "add",
            "c-cmd2",
            "--title",
            "y",
            "--ref-cmd",
            "true",
            "--ref-dir",
            &dir,
            "--output",
            "text",
        ])
        .output()
        .unwrap();
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(
        s.contains("notice:") && s.contains("--ref-report"),
        "human add must render the steering notice: {s}"
    );

    // readiness drift profile, from the fixed set.
    let r = data(&d, &["readiness"]);
    let dp = r["drift_profiles"]
        .as_array()
        .expect("drift_profiles array");
    let allowed = [
        "live_resolved",
        "cached_command",
        "stale",
        "unresolved",
        "asserted",
    ];
    for e in dp {
        assert!(
            allowed.contains(&e["profile"].as_str().unwrap()),
            "profile must be from the fixed set: {e}"
        );
    }
    assert!(
        dp.iter()
            .any(|e| e["id"] == "c-cmd" && e["profile"] == "live_resolved"),
        "default command ref must profile live_resolved: {dp:?}"
    );
    assert!(
        dp.iter()
            .any(|e| e["id"] == "c-rpt" && e["profile"] == "live_resolved"),
        "report-backed control must profile live_resolved: {dp:?}"
    );
}

/// SC-4: a file_anchor control surfaces the age of its source artifact (mtime).
#[test]
fn sc4_source_artifact_age_surfaced() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    init(&d);
    let fresh = write_src(&tmp, "fresh.toml", "[r.r1]\nstatus = \"met\"\n");
    data(
        &d,
        &[
            "control",
            "add",
            "c-fresh",
            "--title",
            "x",
            "--ref-file",
            &fresh,
            "--ref-anchor",
            "r.r1.status",
        ],
    );
    let c = data(&d, &["control", "show", "c-fresh"]);
    let age = c["resolution"]["source_age_secs"]
        .as_i64()
        .expect("source_age_secs present for file_anchor");
    assert!((0..60).contains(&age), "fresh source age ~0: {age}");

    // Back-date the source mtime → the surfaced age grows visibly.
    let old = write_src(&tmp, "old.toml", "[r.r1]\nstatus = \"met\"\n");
    let _ = std::process::Command::new("touch")
        .args(["-t", "202001010000", &old])
        .status();
    data(
        &d,
        &[
            "control",
            "add",
            "c-old",
            "--title",
            "x",
            "--ref-file",
            &old,
            "--ref-anchor",
            "r.r1.status",
        ],
    );
    let c2 = data(&d, &["control", "show", "c-old"]);
    let old_age = c2["resolution"]["source_age_secs"].as_i64().unwrap();
    assert!(
        old_age > 60 * 60 * 24 * 365,
        "back-dated source must show a large age: {old_age}"
    );
    // Human surface mirrors the source age.
    let out = muster(&d)
        .args(["control", "show", "c-old", "--output", "text"])
        .output()
        .unwrap();
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("src_age="), "human must show source age: {s}");
}

/// b2: an opt-in `MUSTER_SOURCE_FRESHNESS_SECS` bound flags a live-passing
/// file_anchor whose SOURCE artifact is stale (mtime older than the bound) — the
/// verdict resolved live but derives from an un-regenerated file, so it is NOT
/// fresh coverage. Default (no bound) keeps today's behavior exactly.
#[test]
fn b2_source_freshness_bound_flags_stale_source() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    init(&d);
    // A passing file_anchor whose source mtime is back-dated to 2020.
    let src = write_src(&tmp, "cov.toml", "[r.r1]\nstatus = \"met\"\n");
    let _ = std::process::Command::new("touch")
        .args(["-t", "202001010000", &src])
        .status();
    data(
        &d,
        &[
            "control",
            "add",
            "c-cov",
            "--title",
            "Coverage",
            "--ref-file",
            &src,
            "--ref-anchor",
            "r.r1.status",
        ],
    );

    // No bound (default): the live `met` counts as covered; no source-stale gap.
    let r0 = data(&d, &["readiness"]);
    assert_eq!(r0["control_coverage"]["implemented_with_evidence"], 1);
    assert!(
        !r0["gap_findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|g| g["kind"] == "ref_source_stale"),
        "no bound ⇒ no source-stale gating"
    );

    // With a 1-hour bound, the 2020 artifact is stale-by-source: not fresh
    // coverage, a ref_source_stale finding, verdict not READY.
    let out = muster(&d)
        .env("MUSTER_SOURCE_FRESHNESS_SECS", "3600")
        .args(["readiness", "--output", "json"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    let r = &v["data"];
    assert_eq!(
        r["control_coverage"]["implemented_with_evidence"], 0,
        "a stale source is not fresh coverage"
    );
    assert!(
        r["gap_findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|g| g["kind"] == "ref_source_stale" && g["subject_id"] == "c-cov"),
        "a stale source must surface a ref_source_stale finding: {}",
        r["gap_findings"]
    );
    assert_ne!(r["verdict"], "READY");
}

/// b3: with `MUSTER_CMD_CACHE` on, command-ref verdicts are served from a cache
/// and may drift — `readiness` must warn (and carry a machine flag) so the
/// operator knows the honesty guarantee is weakened. Default (off): no warning.
#[test]
fn b3_readiness_warns_in_cmd_cache_mode() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    init(&d);
    // default: no cache mode → flag false, no warning in the human surface.
    let r = data(&d, &["readiness"]);
    assert_eq!(r["cmd_cache_mode"], false);
    let plain = muster(&d)
        .args(["readiness", "--output", "text"])
        .output()
        .unwrap();
    assert!(
        !String::from_utf8_lossy(&plain.stdout).contains("command-cache mode"),
        "default must not warn"
    );

    // cache mode on → flag true + a human warning naming the env var.
    let out = muster(&d)
        .env("MUSTER_CMD_CACHE", "1")
        .args(["readiness", "--output", "json"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["data"]["cmd_cache_mode"], true);
    let txt = muster(&d)
        .env("MUSTER_CMD_CACHE", "1")
        .args(["readiness", "--output", "text"])
        .output()
        .unwrap();
    let s = String::from_utf8_lossy(&txt.stdout);
    assert!(
        s.contains("command-cache mode") && s.contains("MUSTER_CMD_CACHE"),
        "cache mode must warn and name the env var: {s}"
    );
}

/// b3: the doctor surface (`control resolve --all`) likewise warns in cache mode.
#[test]
fn b3_resolve_all_warns_in_cmd_cache_mode() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    init(&d);
    // default (no cache mode): flag false, no warning in the human surface.
    let off = data(&d, &["control", "resolve", "--all"]);
    assert_eq!(off["cmd_cache_mode"], false);
    let plain = muster(&d)
        .args(["control", "resolve", "--all", "--output", "text"])
        .output()
        .unwrap();
    assert!(
        !String::from_utf8_lossy(&plain.stdout).contains("command-cache mode"),
        "default must not warn"
    );

    let out = muster(&d)
        .env("MUSTER_CMD_CACHE", "1")
        .args(["control", "resolve", "--all", "--output", "json"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["data"]["cmd_cache_mode"], true);
    let txt = muster(&d)
        .env("MUSTER_CMD_CACHE", "1")
        .args(["control", "resolve", "--all", "--output", "text"])
        .output()
        .unwrap();
    let s = String::from_utf8_lossy(&txt.stdout);
    assert!(
        s.contains("command-cache mode") && s.contains("MUSTER_CMD_CACHE"),
        "doctor surface must warn in cache mode: {s}"
    );
}

/// SC-5: `control resolve --all` re-resolves every ref-backed control and flags
/// any that silently went Unresolved; usage is exactly-one-of id/--all.
#[test]
fn sc5_resolve_all_flags_unresolved() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    init(&d);
    let src = write_src(&tmp, "s.toml", "[r.r1]\nstatus = \"met\"\n");
    data(
        &d,
        &[
            "control",
            "add",
            "c1",
            "--title",
            "x",
            "--ref-file",
            &src,
            "--ref-anchor",
            "r.r1.status",
        ],
    );
    let rep = data(&d, &["control", "resolve", "--all"]);
    assert_eq!(rep["unresolved_count"], 0);
    assert!(
        rep["resolved"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["id"] == "c1" && e["resolution_state"] == "derived"),
        "resolve --all must list c1 derived: {rep}"
    );

    // Refactor the source so the anchor is gone → c1 silently went Unresolved.
    std::fs::write(&src, "[r.other]\nx = 1\n").unwrap();
    let rep2 = data(&d, &["control", "resolve", "--all"]);
    assert_eq!(rep2["unresolved_count"], 1, "report: {rep2}");
    assert!(
        rep2["resolved"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["id"] == "c1" && e["unresolved"] == true),
        "resolve --all must flag c1 unresolved: {rep2}"
    );

    // Usage guards: not both, not neither.
    let (ok, _) = run_json(&d, &["control", "resolve", "c1", "--all"]);
    assert!(!ok, "id + --all must be refused");
    let (ok2, _) = run_json(&d, &["control", "resolve"]);
    assert!(!ok2, "neither id nor --all must be refused");
}

/// SC-7: readiness traversal terminates deterministically even when ref-backed
/// controls sit on processes that form a graph cycle (no infinite loop).
#[test]
fn sc7_cycle_safety_readiness_terminates() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    init(&d);
    let src = write_src(&tmp, "s.toml", "[r.r1]\nstatus = \"met\"\n");
    data(&d, &["process", "add", "p1", "--name", "P1"]);
    data(&d, &["process", "add", "p2", "--name", "P2"]);
    data(
        &d,
        &[
            "process",
            "step",
            "add",
            "p1",
            "--description",
            "x",
            "--process-ref",
            "p2",
        ],
    );
    data(
        &d,
        &[
            "process",
            "step",
            "add",
            "p2",
            "--description",
            "x",
            "--process-ref",
            "p1",
        ],
    );
    data(
        &d,
        &[
            "control",
            "add",
            "c1",
            "--title",
            "x",
            "--ref-file",
            &src,
            "--ref-anchor",
            "r.r1.status",
        ],
    );
    data(&d, &["process", "link-control", "p1", "c1"]);
    // Must terminate and report the cycle deterministically.
    let r = data(&d, &["readiness"]);
    assert!(
        !r["cycles"].as_array().unwrap().is_empty(),
        "cycle must be detected: {r}"
    );
    let r2 = data(&d, &["readiness"]);
    assert_eq!(r, r2, "readiness output must be deterministic");
}

/// SC-7 / Manifesto #7: a live-resolved ref (file_anchor; command in default
/// mode) persists NO authoritative `resolved` copy on disk — truth is read from
/// the source, not a stored excerpt.
#[test]
fn ssot_live_refs_persist_no_resolved_cache() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    init(&d);
    let src = write_src(&tmp, "s.toml", "[r.r1]\nstatus = \"met\"\n");
    data(
        &d,
        &[
            "control",
            "add",
            "c1",
            "--title",
            "x",
            "--ref-file",
            &src,
            "--ref-anchor",
            "r.r1.status",
        ],
    );
    let body = std::fs::read_to_string(d.join("controls").join("c1.json")).unwrap();
    let v: Value = serde_json::from_str(&body).unwrap();
    assert!(
        v.get("resolved").is_none(),
        "live file_anchor must not persist a resolved copy: {body}"
    );
}
