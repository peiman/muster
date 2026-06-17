// OUT-005 exception: test skip-signal writes to stderr are legitimate test-harness
// communication (not library output). The Output struct cannot be used here.
#![allow(clippy::print_stderr)]
//! `ckeletin-doctor` smoke test.
//!
//! The doctor is an environment diagnostic: it reports the framework version and
//! the toolchain + tools the framework depends on. It is INFORMATIONAL — it must
//! exit 0 even when a tool is missing (it reports status, it does not gate the
//! build), so it is deliberately NOT part of `just check`. This test asserts it
//! runs and surfaces the key sections. Unlike the update self-guard, the doctor
//! is not upstream-specific, so it also runs cleanly inside an init'd project.

use std::process::Command;

fn workspace_root() -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR"); // .ckeletin/crate
    std::path::Path::new(manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .unwrap()
        .to_str()
        .unwrap()
        .to_string()
}

fn have(cmd: &str) -> bool {
    Command::new(cmd)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[test]
fn doctor_reports_environment_and_never_fails() {
    if !have("just") {
        eprintln!("SKIP doctor: `just` not on PATH");
        return;
    }

    let out = Command::new("just")
        .arg("ckeletin-doctor")
        .current_dir(workspace_root())
        .output()
        .expect("failed to run `just ckeletin-doctor`");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    assert!(
        out.status.success(),
        "ckeletin-doctor is informational and must exit 0.\nstdout: {stdout}\nstderr: {stderr}"
    );

    // Key sections the doctor must surface.
    for expect in [
        "ckeletin framework v",
        "Toolchain",
        "Tools",
        "cargo-deny",
        "just",
    ] {
        assert!(
            stdout.contains(expect),
            "doctor output missing {expect:?}.\nstdout: {stdout}\nstderr: {stderr}"
        );
    }
}

#[test]
fn doctor_json_is_machine_readable() {
    // For autonomous operation (workhorse driving ckeletin), `ckeletin-doctor
    // json` must emit a single valid JSON object an agent can parse — framework
    // version, toolchain, and tool presence as booleans.
    if !have("just") {
        eprintln!("SKIP doctor json: `just` not on PATH");
        return;
    }

    let out = Command::new("just")
        .args(["ckeletin-doctor", "json"])
        .current_dir(workspace_root())
        .output()
        .expect("failed to run `just ckeletin-doctor json`");

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "doctor json must exit 0.\nstdout: {stdout}"
    );

    // Must be exactly one valid JSON object.
    let v: serde_json::Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("doctor json is not valid JSON: {e}\nstdout: {stdout}"));

    assert!(
        v.get("framework_version")
            .and_then(|x| x.as_str())
            .is_some(),
        "json missing framework_version: {v}"
    );
    assert!(
        v.pointer("/toolchain/pinned")
            .and_then(|x| x.as_str())
            .is_some(),
        "json missing toolchain.pinned: {v}"
    );
    // tool presence must be booleans an agent can branch on.
    assert!(
        v.pointer("/tools/cargo-deny")
            .and_then(|x| x.as_bool())
            .is_some(),
        "json missing boolean tools.cargo-deny: {v}"
    );

    // rustfmt and clippy components (required by `just check`) must appear in
    // JSON parity with the text mode that has always reported them.
    assert!(
        v.pointer("/components/rustfmt")
            .and_then(|x| x.as_bool())
            .is_some(),
        "json missing boolean components.rustfmt: {v}"
    );
    assert!(
        v.pointer("/components/clippy")
            .and_then(|x| x.as_bool())
            .is_some(),
        "json missing boolean components.clippy: {v}"
    );
}

#[test]
fn doctor_json_and_text_report_same_components() {
    // Parity test: the text mode and JSON mode must both report rustfmt and
    // clippy component presence. This catches the case where one mode is
    // updated without the other (finding #10 from the 2026-06-09 code review).
    if !have("just") {
        eprintln!("SKIP doctor_json_and_text_report_same_components: `just` not on PATH");
        return;
    }

    let root = workspace_root();

    let text_out = Command::new("just")
        .arg("ckeletin-doctor")
        .current_dir(&root)
        .output()
        .expect("failed to run `just ckeletin-doctor`");
    let text_stdout = String::from_utf8_lossy(&text_out.stdout);

    let json_out = Command::new("just")
        .args(["ckeletin-doctor", "json"])
        .current_dir(&root)
        .output()
        .expect("failed to run `just ckeletin-doctor json`");
    let json_stdout = String::from_utf8_lossy(&json_out.stdout);

    let v: serde_json::Value = serde_json::from_str(json_stdout.trim())
        .unwrap_or_else(|e| panic!("doctor json is not valid JSON: {e}\njson: {json_stdout}"));

    // Text mode must mention both components.
    assert!(
        text_stdout.contains("rustfmt component"),
        "text mode must mention 'rustfmt component'.\nstdout: {text_stdout}"
    );
    assert!(
        text_stdout.contains("clippy component"),
        "text mode must mention 'clippy component'.\nstdout: {text_stdout}"
    );

    // JSON must have components.rustfmt and components.clippy as booleans.
    let rustfmt_json = v.pointer("/components/rustfmt").and_then(|x| x.as_bool());
    let clippy_json = v.pointer("/components/clippy").and_then(|x| x.as_bool());

    assert!(
        rustfmt_json.is_some(),
        "json must have boolean /components/rustfmt.\njson: {v}"
    );
    assert!(
        clippy_json.is_some(),
        "json must have boolean /components/clippy.\njson: {v}"
    );

    // Both modes must agree on installed/not-installed for both components.
    // Text mode says "installed" vs "NOT FOUND", JSON mode uses true/false.
    let rustfmt_text_installed = text_stdout.contains("rustfmt component: installed");
    let clippy_text_installed = text_stdout.contains("clippy component: installed");

    assert_eq!(
        rustfmt_text_installed,
        rustfmt_json.unwrap(),
        "rustfmt presence disagrees between text ({rustfmt_text_installed}) and json ({:?})",
        rustfmt_json
    );
    assert_eq!(
        clippy_text_installed,
        clippy_json.unwrap(),
        "clippy presence disagrees between text ({clippy_text_installed}) and json ({:?})",
        clippy_json
    );
}
