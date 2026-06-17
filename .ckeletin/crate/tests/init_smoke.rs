//! Init smoke test — copies the scaffold to a temp directory, runs init,
//! and verifies the result is a committed git repo whose full test suite
//! passes.
//!
//! This is the most important test in the scaffold. If it passes, new
//! projects work out of the box. If it fails, `just init` is broken.
//!
//! Regression guard for https://github.com/peiman/ckeletin-rust/issues/1:
//!   - init MUST leave a project that COMPILES. It keeps the `ping` worked
//!     example (matching the ckeletin-go scaffold) rather than stripping the
//!     only subcommand and leaving an empty `Commands` enum that the entry
//!     point cannot match exhaustively.
//!   - init MUST leave a project that is a git repo tagged `v0.0.0`.
//!
//! Crucially it runs `cargo test --workspace` on the initialized project.
//! init.sh's own verification uses `cargo check --workspace --all-targets`
//! (lib, bin, AND test targets), so a broken integration-test file is caught
//! immediately rather than waiting for the user's first `just check`. This
//! test adds the extra guarantee of actually RUNNING the tests, not just
//! compiling them.
//!
//! Ignored by default because it's slow (a full from-scratch build of the
//! initialized project). Wired into CI as a dedicated job and runnable
//! locally via `just init-smoke`.
//!
//! Run explicitly: cargo test -p ckeletin --test init_smoke -- --ignored

use std::process::Command;

/// Find the workspace root (parent of .ckeletin/crate/).
fn workspace_root() -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    // manifest_dir is .ckeletin/crate, workspace root is two levels up
    std::path::Path::new(manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .unwrap()
        .to_str()
        .unwrap()
        .to_string()
}

#[test]
#[ignore] // slow — run explicitly or via the CI init-smoke job
fn init_produces_committed_compilable_project() {
    let root = workspace_root();
    let tmp = tempfile::tempdir().unwrap();
    let project_dir = tmp.path().join("testproject");

    // Copy the scaffold (excluding .git and target).
    let status = Command::new("rsync")
        .args([
            "-a",
            "--exclude=.git",
            "--exclude=target",
            &format!("{}/", root),
            project_dir.to_str().unwrap(),
        ])
        .status()
        .expect("rsync failed");
    assert!(status.success(), "rsync copy failed");

    // Initialize as "testproject". Provide a hermetic git identity so the
    // scaffold's initial commit + tag succeed regardless of the host's git
    // config (CI runners have none).
    let init = Command::new("bash")
        .arg(".ckeletin/scripts/init.sh")
        .arg("testproject")
        .current_dir(&project_dir)
        .env("GIT_AUTHOR_NAME", "ckeletin smoke")
        .env("GIT_AUTHOR_EMAIL", "smoke@ckeletin.test")
        .env("GIT_COMMITTER_NAME", "ckeletin smoke")
        .env("GIT_COMMITTER_EMAIL", "smoke@ckeletin.test")
        .output()
        .expect("init.sh failed to execute");

    assert!(
        init.status.success(),
        "init.sh failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&init.stdout),
        String::from_utf8_lossy(&init.stderr),
    );

    // init MUST leave a git repo with the initial v0.0.0 tag — not an
    // un-versioned, un-committed directory.
    assert!(
        project_dir.join(".git").is_dir(),
        "init.sh did not initialize a git repository"
    );
    let tags = Command::new("git")
        .args(["tag", "--list"])
        .current_dir(&project_dir)
        .output()
        .expect("git tag failed");
    assert!(
        String::from_utf8_lossy(&tags.stdout).contains("v0.0.0"),
        "init.sh did not create the v0.0.0 tag"
    );

    // No stale "ckeletin-rust" references in project source.
    let grep = Command::new("grep")
        .args([
            "-r",
            "ckeletin-rust",
            "--include=*.rs",
            "--include=*.toml",
            "crates/",
        ])
        .current_dir(&project_dir)
        .output()
        .unwrap();
    let stale = String::from_utf8_lossy(&grep.stdout);
    assert!(
        stale.is_empty(),
        "Found stale 'ckeletin-rust' references after init:\n{stale}"
    );

    // Binary name and env prefix were patched.
    let cli_toml = std::fs::read_to_string(project_dir.join("crates/cli/Cargo.toml")).unwrap();
    assert!(
        cli_toml.contains("name = \"testproject\""),
        "Binary name not set in cli/Cargo.toml"
    );
    let main_rs = std::fs::read_to_string(project_dir.join("crates/cli/src/main.rs")).unwrap();
    assert!(
        main_rs.contains("\"TESTPROJECT_\""),
        "Env prefix not patched in main.rs"
    );

    // The worked-example `ping` command is KEPT (renamed), matching the
    // ckeletin-go scaffold. Stripping it to an empty `Commands` enum is what
    // produced a non-compiling project in issue #1.
    assert!(
        project_dir.join("crates/domain/src/ping.rs").exists(),
        "ping.rs (worked example) should be retained after init"
    );

    // Scaffold-leftover guard: the initialized project has no upstream
    // fingerprint, so it runs in consumer mode. Assert ZERO leftovers remain
    // in functional files — this catches any scaffolded file that init.sh
    // forgot to substitute AND any new file that bakes in the scaffold's
    // identity without a derivation. This gate makes "scaffold gained a new
    // identity-bearing functional file that nothing substitutes or derives"
    // a PR-time failure in THIS repo. If this assertion fires, fix the
    // leftover (derivation preferred, init.sh substitution second) until it
    // is green — do not weaken the scan.
    let scan_result = ckeletin::scaffold_scan::scan_for_leftovers(&project_dir);
    match &scan_result {
        ckeletin::scaffold_scan::ScanOutcome::Upstream => {
            panic!(
                "scaffold_scan reported Upstream for the initialized project — init.sh must have failed to rewrite the upstream slug in Cargo.toml"
            );
        }
        ckeletin::scaffold_scan::ScanOutcome::Leftovers(hits) => {
            let report = hits.join("\n  ");
            panic!(
                "scaffold-leftover guard: the initialized project still contains \
                 `ckeletin-rust` literals in functional files. Fix init.sh or \
                 switch the file to a derivation-based approach:\n  {report}"
            );
        }
        ckeletin::scaffold_scan::ScanOutcome::Clean => {
            // No leftovers — gate passes.
        }
    }

    // The strongest signal: the initialized project's full test suite
    // compiles and passes. This compiles test targets — which init.sh's own
    // `cargo check` does not exercise — so a mangled integration-test file
    // cannot slip through to the user.
    let test = Command::new("cargo")
        .args(["test", "--workspace"])
        .current_dir(&project_dir)
        .output()
        .expect("cargo test failed to execute");
    assert!(
        test.status.success(),
        "initialized project's test suite failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&test.stdout),
        String::from_utf8_lossy(&test.stderr),
    );
}
