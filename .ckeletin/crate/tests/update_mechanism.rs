// OUT-005 exception: test skip-signal writes to stderr are legitimate test-harness
// communication (not library output). The Output struct cannot be used here.
#![allow(clippy::print_stderr)]
//! Hermetic update-mechanism tests.
//!
//! These tests build minimal consumer + upstream git fixtures in temp dirs and
//! exercise the real `just ckeletin-update` / `ckeletin-update-check-compatibility`
//! shell recipes through real git operations — no string-assertion about the
//! Justfile text, only observed file-system and process outcomes.
//!
//! ## What is tested here (findings from 2026-06-09 code review)
//!
//! ### Finding #1 — wholesale replacement (HIGH)
//! `git checkout <ref> -- .ckeletin/` does NOT delete files that are absent from
//! the ref.  The fix is `git restore --source=<ref> --staged --worktree --
//! .ckeletin/`.  Regression: upstream deletes a file → consumer update removes it.
//!
//! ### Finding #2 — rollback/restore leaves added files staged (HIGH)
//! `git checkout HEAD -- .ckeletin/` does NOT remove staged-but-not-committed
//! files. The fix uses `git restore --source=HEAD --staged --worktree` in both
//! the tier-1 rollback and the check-compatibility restore() trap.
//! Regression (a): check-compatibility across a file-adding release leaves
//! `git status --porcelain .ckeletin/` empty.
//! Regression (b): a forced compile-fail update rolls back cleanly; the
//! CKELETIN_UPDATE_RESULT JSON contains `"rolled_back":true` and is valid.
//!
//! ### Finding #3 — tag-pinned updates (MEDIUM)
//! `ckeletin-upstream/v0.2.0` is not a valid ref (tags are `refs/tags/`, not
//! `refs/remotes/<remote>/`). Fix: fetch the tag explicitly, use FETCH_HEAD.
//! Regression: `ckeletin-update version=<tag>` succeeds when the upstream has
//! that tag.
//!
//! ### Finding #4 — machine contract coverage (MEDIUM)
//! CKELETIN_UPDATE_RESULT must be emitted and parseable on success and
//! compile-failed paths.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

// ── helpers ──────────────────────────────────────────────────────────────────

fn have(cmd: &str) -> bool {
    Command::new(cmd)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn workspace_root() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR"); // .ckeletin/crate
    Path::new(manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .unwrap()
        .to_path_buf()
}

/// Run `git` in `dir`, asserting success. Returns trimmed stdout.
fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap_or_else(|e| panic!("git {args:?} failed to spawn: {e}"));
    assert!(
        out.status.success(),
        "git {args:?} failed (exit {:?})\nstdout: {}\nstderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
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

/// Returns the path of the upstream work tree (sibling of the bare repo).
fn upstream_work(upstream_bare: &Path) -> PathBuf {
    upstream_bare.parent().unwrap().join("upstream_work")
}

/// Build a bare upstream repo at `upstream_bare`.
///
/// Layout of `.ckeletin/`:
/// - `VERSION` = `version`
/// - `sentinel.txt` = `"upstream"` (canary file for deletion tests)
/// - `crate/` — minimal valid Rust workspace member
/// - `Justfile` — stub so consumer `import '.ckeletin/Justfile'` parses
///
/// Strategy:
/// 1. Create a bare repo at `upstream_bare`.
/// 2. Clone it as a normal work tree (`upstream_work`).
/// 3. Populate & commit in the work tree, push to bare — so `origin` in the
///    work tree always refers to the bare clone.
fn make_upstream(upstream_bare: &Path, version: &str) {
    let parent = upstream_bare.parent().unwrap();
    let work = upstream_work(upstream_bare);

    // 1. Create bare repo.
    git(
        parent,
        &[
            "init",
            "--bare",
            "-b",
            "main",
            upstream_bare.to_str().unwrap(),
        ],
    );

    // 2. Clone bare → work tree.
    git(
        parent,
        &[
            "clone",
            upstream_bare.to_str().unwrap(),
            work.to_str().unwrap(),
        ],
    );
    git(&work, &["config", "user.name", "ckeletin test"]);
    git(&work, &["config", "user.email", "test@ckeletin.test"]);

    // 3. Populate.
    let ck = work.join(".ckeletin");
    let crate_dir = ck.join("crate").join("src");
    std::fs::create_dir_all(&crate_dir).unwrap();

    std::fs::write(
        ck.join("crate").join("Cargo.toml"),
        "[package]\nname = \"ckeletin\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    std::fs::write(crate_dir.join("lib.rs"), "// framework\n").unwrap();
    std::fs::write(ck.join("VERSION"), version).unwrap();
    std::fs::write(ck.join("sentinel.txt"), "upstream").unwrap();
    // Stub Justfile — consumer Justfile imports this path; it must exist and
    // be parseable by `just`, but the real recipe-under-test comes from the
    // install_ckeletin_justfile_into() helper which overwrites it.
    std::fs::write(ck.join("Justfile"), "# upstream stub\n").unwrap();

    git(&work, &["add", "-A"]);
    Command::new("git")
        .args(["commit", "-m", "upstream initial"])
        .current_dir(&work)
        .envs(git_env())
        .status()
        .unwrap();
    git(&work, &["push", "origin", "main"]);
}

/// Extend the upstream with a new commit (adds/updates/deletes files under the
/// work tree, then pushes to the bare repo).
fn upstream_add_commit(upstream_bare: &Path, msg: &str, add: &[(&str, &str)], delete: &[&str]) {
    let work = upstream_work(upstream_bare);
    for (rel, content) in add {
        let dest = work.join(rel);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&dest, content).unwrap();
    }
    for rel in delete {
        let _ = std::fs::remove_file(work.join(rel));
        git(&work, &["rm", "--ignore-unmatch", rel]);
    }
    git(&work, &["add", "-A"]);
    Command::new("git")
        .args(["commit", "-m", msg])
        .current_dir(&work)
        .envs(git_env())
        .status()
        .unwrap();
    git(&work, &["push", "origin", "main"]);
}

/// Tag the current HEAD of the upstream work tree and push the tag to the bare.
///
/// Uses `-c tag.gpgSign=false` so the test works on hosts with GPG signing
/// enabled globally (e.g. peiman's dev machine with `tag.gpgSign = true`).
fn upstream_tag(upstream_bare: &Path, tag: &str) {
    let work = upstream_work(upstream_bare);
    // Lightweight tag: pass -c tag.gpgSign=false to override host config.
    let out = Command::new("git")
        .args(["-c", "tag.gpgSign=false", "tag", tag])
        .current_dir(&work)
        .envs(git_env())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git tag {tag} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    git(&work, &["push", "origin", tag]);
}

/// Build a minimal consumer project in `consumer_path`.
///
/// The consumer has:
/// - Root Cargo.toml (workspace, name rewritten away from `peiman/ckeletin-rust`)
/// - `.ckeletin/VERSION` (initial version)
/// - `.ckeletin/sentinel.txt` (present from the initial scaffold)
/// - A minimal `.ckeletin/Justfile`
/// - A `ckeletin-upstream` remote pointing at `upstream_bare`
/// - One committed state (so rollback has a `HEAD` to restore to)
fn make_consumer(consumer_path: &Path, upstream_bare: &Path, initial_version: &str) {
    std::fs::create_dir_all(consumer_path).unwrap();
    git(consumer_path, &["init", "-b", "main"]);
    git(consumer_path, &["config", "user.name", "ckeletin test"]);
    git(
        consumer_path,
        &["config", "user.email", "test@ckeletin.test"],
    );

    // Workspace Cargo.toml — slug is NOT the upstream slug (simulates an init'd project).
    std::fs::write(
        consumer_path.join("Cargo.toml"),
        "[workspace]\nmembers = [\".ckeletin/crate\"]\n\n[workspace.metadata]\nrepository = \"https://github.com/peiman/testproject\"\n",
    )
    .unwrap();

    // Bootstrap .ckeletin/ matching the upstream's current state.
    let ck = consumer_path.join(".ckeletin");
    std::fs::create_dir_all(&ck).unwrap();

    let crate_dir = ck.join("crate");
    std::fs::create_dir_all(crate_dir.join("src")).unwrap();
    std::fs::write(
        crate_dir.join("Cargo.toml"),
        "[package]\nname = \"ckeletin\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    std::fs::write(crate_dir.join("src").join("lib.rs"), "// framework\n").unwrap();

    std::fs::write(ck.join("VERSION"), initial_version).unwrap();
    std::fs::write(ck.join("sentinel.txt"), "upstream").unwrap();
    std::fs::write(
        ck.join("Justfile"),
        "# minimal upstream justfile for tests\n",
    )
    .unwrap();

    git(consumer_path, &["add", "-A"]);
    Command::new("git")
        .args(["commit", "-m", "initial scaffold"])
        .current_dir(consumer_path)
        .envs(git_env())
        .status()
        .unwrap();

    // Wire the upstream remote.
    git(
        consumer_path,
        &[
            "remote",
            "add",
            "ckeletin-upstream",
            upstream_bare.to_str().unwrap(),
        ],
    );
}

/// Parse the CKELETIN_UPDATE_RESULT line from combined stdout+stderr output.
/// Returns the JSON string after the `=` sign.
fn parse_update_result(output: &str) -> serde_json::Value {
    let line = output
        .lines()
        .find(|l| l.starts_with("CKELETIN_UPDATE_RESULT="))
        .unwrap_or_else(|| panic!("CKELETIN_UPDATE_RESULT not found in output:\n{output}"));
    let json_str = line.strip_prefix("CKELETIN_UPDATE_RESULT=").unwrap();
    serde_json::from_str(json_str)
        .unwrap_or_else(|e| panic!("CKELETIN_UPDATE_RESULT is not valid JSON: {e}\nLine: {line}"))
}

// ── Finding #1 — wholesale replacement ────────────────────────────────────────

/// After an update that deletes `.ckeletin/sentinel.txt` upstream, the consumer
/// must NOT have that file in its working tree.
#[test]
fn update_deletes_files_removed_upstream() {
    if !have("just") || !have("rsync") {
        let ci = std::env::var("CI").unwrap_or_default();
        if ci == "true" {
            panic!("FAIL update_deletes_files_removed_upstream: `just`/`rsync` required on CI");
        }
        eprintln!("SKIP update_deletes_files_removed_upstream: `just`/`rsync` not on PATH");
        return;
    }

    let tmp = tempfile::tempdir().unwrap();
    let upstream = tmp.path().join("upstream.git");
    let consumer = tmp.path().join("consumer");

    // Build upstream at v1 (has sentinel.txt).
    make_upstream(&upstream, "1.0.0");
    // Build consumer pointing at upstream.
    make_consumer(&consumer, &upstream, "1.0.0");

    // Upstream deletes sentinel.txt and bumps to v2.
    upstream_add_commit(
        &upstream,
        "v2: delete sentinel.txt",
        &[(".ckeletin/VERSION", "2.0.0")],
        &[".ckeletin/sentinel.txt"],
    );

    // Consumer runs `just ckeletin-update` pointing at the real ckeletin-rust
    // Justfile (we copy it into the consumer for its update recipe).
    install_ckeletin_justfile_into(&consumer);

    let out = Command::new("just")
        .args(["ckeletin-update"])
        .current_dir(&consumer)
        .envs(git_env())
        .output()
        .unwrap_or_else(|e| panic!("just ckeletin-update failed to spawn: {e}"));

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let combined = format!("{stdout}\n{stderr}");

    assert!(
        out.status.success(),
        "ckeletin-update should succeed when compile passes\nstdout: {stdout}\nstderr: {stderr}"
    );

    // The deleted file MUST be gone from the consumer working tree.
    assert!(
        !consumer.join(".ckeletin/sentinel.txt").exists(),
        "sentinel.txt should have been deleted by the wholesale update, but it still exists"
    );

    // VERSION was updated.
    let ver = std::fs::read_to_string(consumer.join(".ckeletin/VERSION")).unwrap();
    assert_eq!(ver.trim(), "2.0.0", "VERSION should be 2.0.0 after update");

    // Machine contract: CKELETIN_UPDATE_RESULT emitted and parseable (finding #4).
    let result = parse_update_result(&combined);
    assert_eq!(result["status"], "updated", "status should be 'updated'");
    assert_eq!(result["committed"], true, "committed should be true");
    assert_eq!(result["rolled_back"], false, "rolled_back should be false");
}

// ── Finding #2a — check-compatibility leaves no staged files ─────────────────

/// After `ckeletin-update-check-compatibility` across a file-ADDING upstream
/// release, `git status --porcelain .ckeletin/` must be empty (no staged files
/// left behind).
#[test]
fn check_compatibility_leaves_no_staged_files() {
    if !have("just") || !have("rsync") {
        let ci = std::env::var("CI").unwrap_or_default();
        if ci == "true" {
            panic!(
                "FAIL check_compatibility_leaves_no_staged_files: `just`/`rsync` required on CI"
            );
        }
        eprintln!("SKIP check_compatibility_leaves_no_staged_files: `just`/`rsync` not on PATH");
        return;
    }

    let tmp = tempfile::tempdir().unwrap();
    let upstream = tmp.path().join("upstream.git");
    let consumer = tmp.path().join("consumer");

    make_upstream(&upstream, "1.0.0");
    make_consumer(&consumer, &upstream, "1.0.0");

    // Upstream ADDS a new file (common case for every framework release).
    upstream_add_commit(
        &upstream,
        "v2: add newfile.txt",
        &[
            (".ckeletin/VERSION", "2.0.0"),
            (".ckeletin/newfile.txt", "new content"),
        ],
        &[],
    );

    install_ckeletin_justfile_into(&consumer);

    let out = Command::new("just")
        .arg("ckeletin-update-check-compatibility")
        .current_dir(&consumer)
        .output()
        .unwrap_or_else(|e| panic!("check-compatibility failed to spawn: {e}"));

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    // check-compatibility exits 0 when compatible.
    assert!(
        out.status.success(),
        "check-compatibility should pass\nstdout: {stdout}\nstderr: {stderr}"
    );

    // Working tree must be clean after the restore() trap fires.
    let porcelain = git(&consumer, &["status", "--porcelain", ".ckeletin/"]);
    assert!(
        porcelain.is_empty(),
        "git status --porcelain .ckeletin/ must be empty after check-compatibility,\n\
         but got:\n{porcelain}"
    );

    // The added file must NOT be present in the working tree.
    assert!(
        !consumer.join(".ckeletin/newfile.txt").exists(),
        "newfile.txt must not persist in the consumer after check-compatibility"
    );
}

// ── Finding #2b — compile-fail rollback is genuine ───────────────────────────

/// When `ckeletin-update` triggers tier-1 rollback (upstream introduced a
/// compile error), the working tree is restored to a pristine state AND the
/// CKELETIN_UPDATE_RESULT JSON contains `"rolled_back":true`.
#[test]
fn update_compile_fail_rolls_back_cleanly() {
    if !have("just") || !have("rsync") {
        let ci = std::env::var("CI").unwrap_or_default();
        if ci == "true" {
            panic!("FAIL update_compile_fail_rolls_back_cleanly: `just`/`rsync` required on CI");
        }
        eprintln!("SKIP update_compile_fail_rolls_back_cleanly: `just`/`rsync` not on PATH");
        return;
    }

    let tmp = tempfile::tempdir().unwrap();
    let upstream = tmp.path().join("upstream.git");
    let consumer = tmp.path().join("consumer");

    make_upstream(&upstream, "1.0.0");
    make_consumer(&consumer, &upstream, "1.0.0");

    // Upstream adds a file AND introduces a compile error in the framework crate.
    upstream_add_commit(
        &upstream,
        "v2: broken compile",
        &[
            (".ckeletin/VERSION", "2.0.0"),
            (".ckeletin/newfile.txt", "this file will be rolled back"),
            // Deliberate syntax error so cargo check --workspace fails.
            (
                ".ckeletin/crate/src/lib.rs",
                "this is not valid rust syntax @@@\n",
            ),
        ],
        &[],
    );

    install_ckeletin_justfile_into(&consumer);

    let out = Command::new("just")
        .arg("ckeletin-update")
        .current_dir(&consumer)
        .envs(git_env())
        .output()
        .unwrap_or_else(|e| panic!("just ckeletin-update failed to spawn: {e}"));

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let combined = format!("{stdout}\n{stderr}");

    // Recipe should exit non-zero (tier-1 rollback path).
    assert!(
        !out.status.success(),
        "ckeletin-update should fail on a compile error\nstdout: {stdout}\nstderr: {stderr}"
    );

    // After rollback the working tree must be clean.
    let porcelain = git(&consumer, &["status", "--porcelain", ".ckeletin/"]);
    assert!(
        porcelain.is_empty(),
        "git status --porcelain .ckeletin/ must be empty after rollback,\n\
         but got:\n{porcelain}"
    );

    // The added newfile.txt must NOT persist.
    assert!(
        !consumer.join(".ckeletin/newfile.txt").exists(),
        "newfile.txt must not persist after a rolled-back update"
    );

    // VERSION must be back to the original.
    let ver = std::fs::read_to_string(consumer.join(".ckeletin/VERSION")).unwrap();
    assert_eq!(
        ver.trim(),
        "1.0.0",
        "VERSION must be restored to 1.0.0 after rollback"
    );

    // Machine contract: CKELETIN_UPDATE_RESULT with rolled_back:true (finding #4).
    let result = parse_update_result(&combined);
    assert_eq!(
        result["status"], "compile_failed",
        "status should be 'compile_failed'"
    );
    assert_eq!(result["rolled_back"], true, "rolled_back must be true");
    assert_eq!(result["committed"], false, "committed must be false");
}

// ── Finding #3 — tag-pinned updates ──────────────────────────────────────────

/// `ckeletin-update version=<tag>` must succeed when the upstream has that tag.
#[test]
fn update_with_explicit_tag_works() {
    if !have("just") || !have("rsync") {
        let ci = std::env::var("CI").unwrap_or_default();
        if ci == "true" {
            panic!("FAIL update_with_explicit_tag_works: `just`/`rsync` required on CI");
        }
        eprintln!("SKIP update_with_explicit_tag_works: `just`/`rsync` not on PATH");
        return;
    }

    // Build upstream at 0.9.0, then publish a tagged v1.0.0 release.
    let tmp = tempfile::tempdir().unwrap();
    let upstream = tmp.path().join("upstream.git");
    let consumer = tmp.path().join("consumer");

    make_upstream(&upstream, "0.9.0");
    make_consumer(&consumer, &upstream, "0.9.0");

    // Upstream publishes v1.0.0 and tags it.
    upstream_add_commit(
        &upstream,
        "v1.0.0 release",
        &[(".ckeletin/VERSION", "1.0.0")],
        &[],
    );
    upstream_tag(&upstream, "v1.0.0");

    install_ckeletin_justfile_into(&consumer);

    // Update consumer to the tagged v1.0.0.
    // `just` accepts positional args for recipe parameters: `just recipe arg`.
    let out = Command::new("just")
        .args(["ckeletin-update", "v1.0.0"])
        .current_dir(&consumer)
        .envs(git_env())
        .output()
        .unwrap_or_else(|e| panic!("just ckeletin-update v1.0.0 failed to spawn: {e}"));

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let combined = format!("{stdout}\n{stderr}");

    assert!(
        out.status.success(),
        "ckeletin-update v1.0.0 should succeed\nstdout: {stdout}\nstderr: {stderr}"
    );

    let ver = std::fs::read_to_string(consumer.join(".ckeletin/VERSION")).unwrap();
    assert_eq!(
        ver.trim(),
        "1.0.0",
        "VERSION should be 1.0.0 after tag-pinned update"
    );

    // Machine contract: success verdict (finding #4).
    let result = parse_update_result(&combined);
    assert_eq!(result["status"], "updated");
}

// ── helper: install the real ckeletin Justfile into a consumer ────────────────

/// Copy the real `.ckeletin/Justfile` into `consumer/.ckeletin/Justfile` so that
/// `just ckeletin-update` in the consumer runs the actual recipe under test,
/// and create a minimal root Justfile that imports it.
fn install_ckeletin_justfile_into(consumer: &Path) {
    let root = workspace_root();
    let src = root.join(".ckeletin/Justfile");
    let dest = consumer.join(".ckeletin/Justfile");
    std::fs::copy(&src, &dest).unwrap_or_else(|e| panic!("failed to copy .ckeletin/Justfile: {e}"));

    // Minimal root Justfile that imports the framework.
    // The root `check` recipe is needed because ckeletin-update calls `just check`.
    let root_justfile = consumer.join("Justfile");
    std::fs::write(
        &root_justfile,
        "import '.ckeletin/Justfile'\n\ncheck:\n    cargo check --workspace -q\n",
    )
    .unwrap();

    // Commit the updated Justfiles so the working tree is clean before update.
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(consumer)
        .status()
        .unwrap();
    let status = Command::new("git")
        .args(["commit", "-m", "add justfiles"])
        .current_dir(consumer)
        .envs(git_env())
        .status()
        .unwrap();
    // Ignore "nothing to commit" (exit 1) — already committed.
    let _ = status;
}
