// OUT-005 exception: test skip-signal writes to stderr are legitimate test-harness
// communication (not library output). The Output struct cannot be used here.
#![allow(clippy::print_stderr)]
//! Scaffold-leftover guard fixture tests.
//!
//! These tests verify that consumer repos fail `just check` when scaffolded
//! functional files still contain the literal `ckeletin-rust` (the scaffold's
//! own identity). This exact class caused ioguard v0.1.0 to publish zero
//! release artifacts — the binary path in release.yml named the scaffold's
//! binary, not ioguard's.
//!
//! ## Scan scope
//!
//! The guard scans FUNCTIONAL files only:
//! - `.github/workflows/*.yml`
//! - root `Justfile`
//! - `lefthook.yml`
//! - root `Cargo.toml`
//! - `crates/**/Cargo.toml` and layer Cargo.tomls declared in ckeletin-project.toml
//! - `deny.toml`
//!
//! It does NOT scan prose docs (README/AGENTS/CHANGELOG/docs/) — consumers
//! legitimately reference the upstream project by name.
//! It does NOT scan anything under `.ckeletin/` — framework-owned, intentional.
//!
//! ## Exclusions within scanned files
//!
//! - Comment lines (leading `#` after trim) — deliberate references.
//! - Lines containing `github.repository ==` — upstream-only job gating,
//!   kept verbatim by consumers.
//! - Any path under `.ckeletin/` — never scanned.
//!
//! ## Upstream repo
//!
//! Detected by the presence of `peiman/ckeletin-rust` in root `Cargo.toml`
//! (same mechanism the Justfile guards use). On the upstream repo the scan
//! is SKIPPED loudly — the literal is legitimate there.

use std::io::Write as _;

use ckeletin::scaffold_scan;
use tempfile::TempDir;

// ── fixture helpers ────────────────────────────────────────────────────────────

fn make_fixture(files: &[(&str, &str)]) -> TempDir {
    let tmp = tempfile::tempdir().unwrap();
    for (rel, content) in files {
        let path = tmp.path().join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
    }
    tmp
}

// ── Fixture 1: upstream repo → scan skipped ───────────────────────────────────

/// On the upstream repo (root Cargo.toml contains `peiman/ckeletin-rust`) the
/// scan must be skipped loudly. The literal is the self-detection fingerprint
/// and is intentional everywhere in the framework.
#[test]
fn upstream_repo_scan_is_skipped() {
    let ws = make_fixture(&[
        (
            "Cargo.toml",
            r#"[workspace]
repository = "https://github.com/peiman/ckeletin-rust"
"#,
        ),
        // Even if a workflow has the literal, upstream must skip.
        (".github/workflows/release.yml", "  BIN=ckeletin-rust\n"),
    ]);

    let result = scaffold_scan::scan_for_leftovers(ws.path());
    assert!(
        matches!(result, scaffold_scan::ScanOutcome::Upstream),
        "Expected Upstream (skip) for upstream repo, got: {result:?}"
    );
}

// ── Fixture 2: consumer clean → scan passes ───────────────────────────────────

/// A consumer repo with NO `ckeletin-rust` literals in any scanned file:
/// scan returns Clean.
#[test]
fn consumer_clean_scan_passes() {
    let ws = make_fixture(&[
        (
            "Cargo.toml",
            r#"[workspace]
repository = "https://github.com/peiman/myapp"
"#,
        ),
        (
            ".github/workflows/release.yml",
            r#"      - name: Build release binary
        run: cargo build --release --locked
      - name: Resolve binary name
        run: |
          BIN=$(cargo metadata --format-version 1 --no-deps \
            | jq -r '.packages[] | select(.targets[]?.kind[]? == "bin") | .targets[] | select(.kind[] == "bin") | .name' \
            | head -1)
          echo "BIN=$BIN" >> "$GITHUB_ENV"
"#,
        ),
        ("Justfile", "check: test\n    @echo done\n"),
    ]);

    let result = scaffold_scan::scan_for_leftovers(ws.path());
    assert!(
        matches!(result, scaffold_scan::ScanOutcome::Clean),
        "Expected Clean for consumer with no leftovers, got: {result:?}"
    );
}

// ── Fixture 3: consumer with leftover in workflow → fails naming it ────────────

/// A consumer whose release.yml still contains `ckeletin-rust` on a non-comment,
/// non-gating line: scan fails and names the file and line.
#[test]
fn consumer_leftover_in_workflow_fails_with_location() {
    let ws = make_fixture(&[
        (
            "Cargo.toml",
            r#"[workspace]
repository = "https://github.com/peiman/myapp"
"#,
        ),
        (
            ".github/workflows/release.yml",
            r#"      - name: Build release binary
        run: cargo build --release --locked
      - name: Resolve binary name
        run: |
          BIN=ckeletin-rust
          echo "BIN=$BIN" >> "$GITHUB_ENV"
"#,
        ),
    ]);

    let result = scaffold_scan::scan_for_leftovers(ws.path());
    match &result {
        scaffold_scan::ScanOutcome::Leftovers(hits) => {
            assert!(!hits.is_empty(), "Expected at least one hit");
            let combined = hits.join("\n");
            assert!(
                combined.contains("release.yml"),
                "Hit should name the file: {combined}"
            );
            assert!(
                combined.contains("ckeletin-rust"),
                "Hit should contain the literal: {combined}"
            );
        }
        other => panic!("Expected Leftovers, got: {other:?}"),
    }
}

// ── Fixture 4: gating-idiom and comment lines → passes ────────────────────────

/// A consumer whose workflow contains ONLY the upstream-gating idiom
/// (`github.repository == 'peiman/ckeletin-rust'`) and comment lines:
/// those must be excluded, and the scan passes.
#[test]
fn consumer_only_gating_idiom_and_comments_passes() {
    let ws = make_fixture(&[
        (
            "Cargo.toml",
            r#"[workspace]
repository = "https://github.com/peiman/myapp"
"#,
        ),
        (
            ".github/workflows/ci.yml",
            r#"jobs:
  init-smoke:
    name: Scaffold init smoke
    # Upstream-only — true for ckeletin-rust itself, not derived projects.
    if: github.repository == 'peiman/ckeletin-rust' && github.event_name != 'schedule'
    runs-on: ubuntu-latest
"#,
        ),
        (
            ".github/workflows/spec-drift.yml",
            r#"on:
  schedule:
    - cron: '0 8 * * 1'
jobs:
  spec-drift:
    if: github.repository == 'peiman/ckeletin-rust'
    runs-on: ubuntu-latest
"#,
        ),
    ]);

    let result = scaffold_scan::scan_for_leftovers(ws.path());
    assert!(
        matches!(result, scaffold_scan::ScanOutcome::Clean),
        "Expected Clean for consumer with only gating-idiom and comment lines, got: {result:?}"
    );
}

// ── Fixture 5: leftover in Cargo.toml crate name → fails ──────────────────────

/// A consumer whose crates/cli/Cargo.toml still has `name = "ckeletin-rust"`
/// in the [[bin]] section: scan fails and names the file.
#[test]
fn consumer_leftover_in_crate_cargo_toml_fails() {
    let ws = make_fixture(&[
        (
            "Cargo.toml",
            r#"[workspace]
repository = "https://github.com/peiman/myapp"
"#,
        ),
        (
            "crates/cli/Cargo.toml",
            r#"[package]
name = "cli"
version = "0.1.0"

[[bin]]
name = "ckeletin-rust"
path = "src/main.rs"
"#,
        ),
    ]);

    let result = scaffold_scan::scan_for_leftovers(ws.path());
    match &result {
        scaffold_scan::ScanOutcome::Leftovers(hits) => {
            let combined = hits.join("\n");
            assert!(
                combined.contains("Cargo.toml"),
                "Hit should name the Cargo.toml: {combined}"
            );
        }
        other => panic!("Expected Leftovers for crate Cargo.toml leftover, got: {other:?}"),
    }
}

// ── Fixture 6: leftover in root Justfile → fails ──────────────────────────────

/// A consumer whose root Justfile still references `ckeletin-rust` on a
/// non-comment line (not inside .ckeletin/): scan fails.
#[test]
fn consumer_leftover_in_justfile_fails() {
    let ws = make_fixture(&[
        (
            "Cargo.toml",
            r#"[workspace]
repository = "https://github.com/peiman/myapp"
"#,
        ),
        (
            "Justfile",
            r#"import '.ckeletin/Justfile'

check: test
    @echo "All checks passed."

build:
    cargo build --release --bin ckeletin-rust
"#,
        ),
    ]);

    let result = scaffold_scan::scan_for_leftovers(ws.path());
    assert!(
        matches!(result, scaffold_scan::ScanOutcome::Leftovers(_)),
        "Expected Leftovers for root Justfile leftover, got: {result:?}"
    );
}

// ── Fixture 7: ckeletin/ contents not scanned ─────────────────────────────────

/// Files under .ckeletin/ must never be scanned — they are framework-owned
/// and intentionally contain the upstream identity.
#[test]
fn ckeletin_directory_contents_not_scanned() {
    let ws = make_fixture(&[
        (
            "Cargo.toml",
            r#"[workspace]
repository = "https://github.com/peiman/myapp"
"#,
        ),
        // Framework-owned file with ckeletin-rust — must not trigger.
        (
            ".ckeletin/Justfile",
            "ckeletin_upstream_slug := \"peiman/ckeletin-rust\"\n",
        ),
        // Framework-owned script — must not trigger.
        (
            ".ckeletin/scripts/init.sh",
            "grep -q \"peiman/ckeletin-rust\" Cargo.toml\n",
        ),
    ]);

    let result = scaffold_scan::scan_for_leftovers(ws.path());
    assert!(
        matches!(result, scaffold_scan::ScanOutcome::Clean),
        "Expected Clean: .ckeletin/ contents must not be scanned, got: {result:?}"
    );
}

// ── Fixture 8: failure message is actionable ──────────────────────────────────

/// The failure message must name the file:line and tell the consumer what to do.
/// This is the quality bar for an agent to fix it unassisted.
#[test]
fn failure_message_names_file_line_and_action() {
    let ws = make_fixture(&[
        (
            "Cargo.toml",
            r#"[workspace]
repository = "https://github.com/peiman/myapp"
"#,
        ),
        (
            ".github/workflows/release.yml",
            "          BIN=ckeletin-rust\n",
        ),
    ]);

    let result = scaffold_scan::scan_for_leftovers(ws.path());
    let hits = match result {
        scaffold_scan::ScanOutcome::Leftovers(h) => h,
        other => panic!("Expected Leftovers, got: {other:?}"),
    };
    let msg = hits.join("\n");
    // Must name the file.
    assert!(
        msg.contains("release.yml"),
        "Message must name the file: {msg}"
    );
    // Must tell the consumer what to do.
    assert!(
        msg.to_lowercase().contains("replace")
            || msg.to_lowercase().contains("cargo metadata")
            || msg.to_lowercase().contains("project"),
        "Message must explain remediation: {msg}"
    );
}

// ── the real-tree gate (Finding 5, consumer-feedback-2026-06-10.md) ──────────
//
// Everything above runs against hermetic fixtures, which proves the scan LOGIC
// — but a guard that never scans the repo it ships in protects nobody. ioguard
// proved it: `just check` passed green on 0.2.24 while its release.yml still
// contained `target/release/ckeletin-rust`, the exact leftover that ate its
// v0.1.0 release artifacts. This test is the missing call site: it scans the
// REAL workspace this test suite runs in, exactly like arch_allowlist.rs does.
// On the upstream repo it skips (the literal is legitimate there); in every
// consumer repo it is the gate that goes red until the tree is clean.

/// Resolve the workspace root from CARGO_MANIFEST_DIR (.ckeletin/crate).
fn workspace_root() -> std::path::PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR"); // .ckeletin/crate
    std::path::Path::new(manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .unwrap()
        .to_path_buf()
}

#[test]
fn this_repos_real_tree_has_no_scaffold_leftovers() {
    match scaffold_scan::scan_for_leftovers(&workspace_root()) {
        scaffold_scan::ScanOutcome::Upstream => {
            eprintln!(
                "SKIP this_repos_real_tree_has_no_scaffold_leftovers: \
                 upstream repo — the ckeletin-rust literal is legitimate here."
            );
        }
        scaffold_scan::ScanOutcome::Clean => {}
        scaffold_scan::ScanOutcome::Leftovers(hits) => {
            panic!(
                "scaffold identity leftovers found in THIS repo's tree \
                 (this class silently published zero release artifacts for \
                 ioguard v0.1.0 — fix before it fails at release time):\n{}",
                hits.join("\n")
            );
        }
    }
}
