// OUT-005 exception: test skip-signal writes to stderr are legitimate test-harness
// communication (not library output). The Output struct cannot be used here.
#![allow(clippy::print_stderr)]
//! `ckeletin-health` exit-code tests.
//!
//! ## Finding #5 — ckeletin-health exit code (MEDIUM)
//! `ckeletin-health` previously exited 0 even when the workspace was BROKEN
//! (the `|| echo "Workspace: BROKEN"` construct swallows the exit code).
//! Fix: use an if/else that calls `exit 1` on compile failure.
//!
//! Tests:
//! - Clean tree: `just ckeletin-health` exits 0 and prints "Workspace: compiles".
//! - Broken workspace fixture: health exits non-zero and prints "Workspace: BROKEN".

use std::{path::Path, process::Command};

fn workspace_root() -> std::path::PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR"); // .ckeletin/crate
    Path::new(manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .unwrap()
        .to_path_buf()
}

fn have(cmd: &str) -> bool {
    Command::new(cmd)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Git env for hermetic commits (CI runners have no identity).
fn git_env() -> [(&'static str, &'static str); 4] {
    [
        ("GIT_AUTHOR_NAME", "ckeletin test"),
        ("GIT_AUTHOR_EMAIL", "test@ckeletin.test"),
        ("GIT_COMMITTER_NAME", "ckeletin test"),
        ("GIT_COMMITTER_EMAIL", "test@ckeletin.test"),
    ]
}

/// `just ckeletin-health` on the clean upstream tree must exit 0 and report
/// "Workspace: compiles". This is the gate `just check` depends on.
#[test]
fn health_exits_zero_on_clean_tree() {
    if !have("just") {
        let ci = std::env::var("CI").unwrap_or_default();
        if ci == "true" {
            panic!("FAIL health_exits_zero_on_clean_tree: `just` required on CI");
        }
        eprintln!("SKIP health_exits_zero_on_clean_tree: `just` not on PATH");
        return;
    }

    let out = Command::new("just")
        .arg("ckeletin-health")
        .current_dir(workspace_root())
        .output()
        .expect("failed to run `just ckeletin-health`");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    assert!(
        out.status.success(),
        "ckeletin-health must exit 0 on a clean compiling tree.\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stdout.contains("Workspace: compiles"),
        "ckeletin-health must report 'Workspace: compiles'.\nstdout: {stdout}"
    );
}

/// `just ckeletin-health` on a workspace with a compile error must exit non-zero
/// and report "Workspace: BROKEN", so `just check` acts as a real gate.
#[test]
fn health_exits_nonzero_on_broken_workspace() {
    if !have("just") || !have("rsync") {
        let ci = std::env::var("CI").unwrap_or_default();
        if ci == "true" {
            panic!("FAIL health_exits_nonzero_on_broken_workspace: `just`/`rsync` required on CI");
        }
        eprintln!("SKIP health_exits_nonzero_on_broken_workspace: `just`/`rsync` not on PATH");
        return;
    }

    let root = workspace_root();
    let tmp = tempfile::tempdir().unwrap();
    let project_dir = tmp.path().join("broken-workspace");

    // Copy the scaffold (exclude .git and target so it is fast).
    let status = Command::new("rsync")
        .args([
            "-a",
            "--exclude=.git",
            "--exclude=target",
            &format!("{}/", root.to_str().unwrap()),
            project_dir.to_str().unwrap(),
        ])
        .status()
        .expect("rsync failed");
    assert!(status.success(), "rsync copy failed");

    // Initialise a git repo so ckeletin-health can run `git status`.
    Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(&project_dir)
        .status()
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "ckeletin test"])
        .current_dir(&project_dir)
        .status()
        .unwrap();
    Command::new("git")
        .args(["config", "user.email", "test@ckeletin.test"])
        .current_dir(&project_dir)
        .status()
        .unwrap();
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(&project_dir)
        .status()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "scaffold copy"])
        .current_dir(&project_dir)
        .envs(git_env())
        .status()
        .unwrap();

    // Introduce a deliberate compile error into the framework crate.
    let lib_rs = project_dir.join(".ckeletin/crate/src/lib.rs");
    let original = std::fs::read_to_string(&lib_rs).unwrap();
    std::fs::write(&lib_rs, format!("{original}\nthis is not valid rust @@@\n")).unwrap();

    let out = Command::new("just")
        .arg("ckeletin-health")
        .current_dir(&project_dir)
        .output()
        .expect("failed to run `just ckeletin-health`");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    assert!(
        !out.status.success(),
        "ckeletin-health must exit non-zero when workspace is BROKEN.\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stdout.contains("Workspace: BROKEN"),
        "ckeletin-health must report 'Workspace: BROKEN'.\nstdout: {stdout}"
    );
}
