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
            "note",
            "runbook approved",
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
    data(&d, &["control", "attach-evidence", "c1", "note", "x"]);

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
fn sc6_dangling_ref_is_unresolved_and_a_gap() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    init(&d);
    data(
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
    let c = data(&d, &["control", "show", "c1"]);
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
fn sc7_command_ref_goes_stale_at_freshness_zero() {
    let tmp = TempDir::new().unwrap();
    let d = data_dir(&tmp);
    init(&d);
    let dir = tmp.path().to_string_lossy().into_owned();
    data(
        &d,
        &[
            "control",
            "add",
            "c1",
            "--title",
            "x",
            "--ref-cmd",
            "true",
            "--ref-dir",
            &dir,
        ],
    );
    // With freshness 0, the served command cache projects to stale.
    let out = muster(&d)
        .env("MUSTER_FRESHNESS_SECS", "0")
        .args(["control", "show", "c1", "--output", "json"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["data"]["resolution"]["resolution_state"], "stale");
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
