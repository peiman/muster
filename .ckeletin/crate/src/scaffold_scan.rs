// OUT-005 exception: this module intentionally produces no stdout/stderr output.
// Callers receive structured results and render them.
//! Scaffold-leftover guard.
//!
//! Scans the functional files of a consumer repo for residual `ckeletin-rust`
//! literals — the scaffold's own identity that every consumer must rename.
//! Leaving these intact causes silent failures at release time (ioguard v0.1.0
//! published zero artifacts because the release.yml binary path named the
//! scaffold's binary, not ioguard's).
//!
//! ## Upstream detection
//!
//! If root `Cargo.toml` contains `peiman/ckeletin-rust`, this IS the upstream
//! repo and the scan is skipped — the literal is intentional there.
//!
//! ## Scanned files
//!
//! - `.github/workflows/*.yml`
//! - Root `Justfile`
//! - `lefthook.yml`
//! - Root `Cargo.toml`
//! - `crates/**/Cargo.toml` (recursive)
//! - `deny.toml`
//!
//! ## Exclusions within scanned lines
//!
//! - Lines whose trimmed form starts with `#` (comment lines).
//! - Lines containing `github.repository ==` (deliberate upstream-only job
//!   gating that consumers keep verbatim).
//!
//! ## Never scanned
//!
//! Anything under `.ckeletin/` — framework-owned, intentional.

use std::path::{Path, PathBuf};

/// The literal that must not appear in consumer functional files.
pub const SCAFFOLD_IDENTITY: &str = "ckeletin-rust";

/// The upstream fingerprint in root Cargo.toml.
const UPSTREAM_SLUG: &str = "peiman/ckeletin-rust";

/// Outcome of [`scan_for_leftovers`].
#[derive(Debug, PartialEq)]
pub enum ScanOutcome {
    /// This is the upstream repo — scan skipped (the literal is intentional).
    Upstream,
    /// No leftovers found in scanned functional files.
    Clean,
    /// One or more leftovers found. Each entry is an actionable message naming
    /// the file, line number, and what to do.
    Leftovers(Vec<String>),
}

/// Scan `workspace_root` for residual scaffold identity literals.
///
/// Returns [`ScanOutcome::Upstream`] when the root `Cargo.toml` contains the
/// upstream slug (this IS the scaffold, not a consumer).  Otherwise scans all
/// functional files and returns [`ScanOutcome::Clean`] or
/// [`ScanOutcome::Leftovers`].
pub fn scan_for_leftovers(workspace_root: &Path) -> ScanOutcome {
    // Upstream detection: if root Cargo.toml names the upstream slug, skip.
    if is_upstream_repo(workspace_root) {
        return ScanOutcome::Upstream;
    }

    let files = collect_scan_targets(workspace_root);
    let mut hits: Vec<String> = Vec::new();

    for path in &files {
        if let Some(file_hits) = scan_file(path, workspace_root) {
            hits.extend(file_hits);
        }
    }

    if hits.is_empty() {
        ScanOutcome::Clean
    } else {
        ScanOutcome::Leftovers(hits)
    }
}

/// True when the root `Cargo.toml` identifies this as the upstream scaffold.
fn is_upstream_repo(workspace_root: &Path) -> bool {
    let cargo_toml = workspace_root.join("Cargo.toml");
    match std::fs::read_to_string(&cargo_toml) {
        Ok(content) => content.contains(UPSTREAM_SLUG),
        Err(_) => false,
    }
}

/// Collect all functional files to scan under `workspace_root`.
/// Never includes anything under `.ckeletin/`.
fn collect_scan_targets(workspace_root: &Path) -> Vec<PathBuf> {
    let mut targets = Vec::new();

    // Root-level singletons.
    for name in &["Cargo.toml", "Justfile", "lefthook.yml", "deny.toml"] {
        let p = workspace_root.join(name);
        if p.exists() {
            targets.push(p);
        }
    }

    // .github/workflows/*.yml
    let workflows_dir = workspace_root.join(".github").join("workflows");
    if workflows_dir.is_dir()
        && let Ok(entries) = std::fs::read_dir(&workflows_dir)
    {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "yml") {
                targets.push(path);
            }
        }
    }

    // crates/**/Cargo.toml (recursive, depth-2: crates/<name>/Cargo.toml)
    let crates_dir = workspace_root.join("crates");
    if crates_dir.is_dir()
        && let Ok(entries) = std::fs::read_dir(&crates_dir)
    {
        for entry in entries.flatten() {
            let crate_dir = entry.path();
            if crate_dir.is_dir() {
                let cargo_toml = crate_dir.join("Cargo.toml");
                if cargo_toml.exists() {
                    targets.push(cargo_toml);
                }
            }
        }
    }

    targets
}

/// Scan a single file for scaffold identity literals.
///
/// Returns `Some(Vec<String>)` with actionable hit messages if any are found,
/// or `None` if the file is clean.
fn scan_file(path: &Path, workspace_root: &Path) -> Option<Vec<String>> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return None,
    };

    // Compute a relative display path for messages.
    let display = path
        .strip_prefix(workspace_root)
        .unwrap_or(path)
        .display()
        .to_string();

    let mut hits = Vec::new();

    for (i, line) in content.lines().enumerate() {
        let lineno = i + 1;

        // Skip comment lines (leading # after trim).
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }

        // Skip upstream-only job gating lines.
        if line.contains("github.repository ==") {
            continue;
        }

        if line.contains(SCAFFOLD_IDENTITY) {
            hits.push(format!(
                "{display}:{lineno}: found scaffold identity literal `{SCAFFOLD_IDENTITY}` — \
                replace it with your project's name, or copy the current upstream \
                derivation-based version of the file (use `cargo metadata --no-deps` to \
                derive binary names structurally). This exact class caused ioguard v0.1.0 \
                to publish zero release artifacts (see ioguard PR #4 as the worked fix).",
            ));
        }
    }

    if hits.is_empty() { None } else { Some(hits) }
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;
    use tempfile::TempDir;

    fn make_workspace(files: &[(&str, &str)]) -> TempDir {
        let tmp = tempfile::tempdir().unwrap();
        for (rel, content) in files {
            let p = tmp.path().join(rel);
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            let mut f = std::fs::File::create(&p).unwrap();
            f.write_all(content.as_bytes()).unwrap();
        }
        tmp
    }

    #[test]
    fn upstream_detection_by_cargo_toml() {
        let ws = make_workspace(&[(
            "Cargo.toml",
            "repository = \"https://github.com/peiman/ckeletin-rust\"\n",
        )]);
        assert!(is_upstream_repo(ws.path()));
    }

    #[test]
    fn consumer_cargo_toml_not_upstream() {
        let ws = make_workspace(&[(
            "Cargo.toml",
            "repository = \"https://github.com/peiman/myapp\"\n",
        )]);
        assert!(!is_upstream_repo(ws.path()));
    }

    #[test]
    fn missing_cargo_toml_not_upstream() {
        let ws = make_workspace(&[]);
        assert!(!is_upstream_repo(ws.path()));
    }

    #[test]
    fn comment_lines_excluded() {
        let ws = make_workspace(&[
            (
                "Cargo.toml",
                "repository = \"https://github.com/peiman/x\"\n",
            ),
            (
                "Justfile",
                "# This is the ckeletin-rust scaffold\ncheck: test\n",
            ),
        ]);
        let result = scan_for_leftovers(ws.path());
        assert!(
            matches!(result, ScanOutcome::Clean),
            "Comment lines must be excluded: {result:?}"
        );
    }

    #[test]
    fn gating_idiom_lines_excluded() {
        let ws = make_workspace(&[
            (
                "Cargo.toml",
                "repository = \"https://github.com/peiman/x\"\n",
            ),
            (
                ".github/workflows/ci.yml",
                "    if: github.repository == 'peiman/ckeletin-rust'\n",
            ),
        ]);
        let result = scan_for_leftovers(ws.path());
        assert!(
            matches!(result, ScanOutcome::Clean),
            "Gating idiom lines must be excluded: {result:?}"
        );
    }

    #[test]
    fn non_yml_files_in_workflows_not_scanned() {
        let ws = make_workspace(&[
            (
                "Cargo.toml",
                "repository = \"https://github.com/peiman/x\"\n",
            ),
            // .json files in .github/workflows must not be scanned.
            (
                ".github/workflows/data.json",
                "{\"binary\": \"ckeletin-rust\"}\n",
            ),
        ]);
        let result = scan_for_leftovers(ws.path());
        assert!(
            matches!(result, ScanOutcome::Clean),
            "Non-yml files in workflows must not be scanned: {result:?}"
        );
    }

    #[test]
    fn ckeletin_dir_never_scanned() {
        // Even if a file named Justfile exists under .ckeletin/, it must not
        // be collected as a scan target — only the root Justfile is.
        let ws = make_workspace(&[
            (
                "Cargo.toml",
                "repository = \"https://github.com/peiman/x\"\n",
            ),
            (
                ".ckeletin/Justfile",
                "ckeletin_upstream_slug := \"peiman/ckeletin-rust\"\n",
            ),
        ]);
        // collect_scan_targets should not include the .ckeletin/Justfile.
        let targets = collect_scan_targets(ws.path());
        for t in &targets {
            assert!(
                !t.components().any(|c| c.as_os_str() == ".ckeletin"),
                "collect_scan_targets must not include .ckeletin/ paths: {t:?}"
            );
        }
    }

    #[test]
    fn hit_message_contains_file_line_and_action() {
        let ws = make_workspace(&[
            (
                "Cargo.toml",
                "repository = \"https://github.com/peiman/x\"\n",
            ),
            (
                ".github/workflows/release.yml",
                "line1\n          BIN=ckeletin-rust\nline3\n",
            ),
        ]);
        let result = scan_for_leftovers(ws.path());
        let hits = match result {
            ScanOutcome::Leftovers(h) => h,
            other => panic!("Expected Leftovers: {other:?}"),
        };
        let msg = hits[0].as_str();
        assert!(msg.contains("release.yml"), "must name file: {msg}");
        assert!(msg.contains(":2:"), "must include line number: {msg}");
        assert!(
            msg.contains("replace") || msg.contains("cargo metadata"),
            "must explain action: {msg}"
        );
    }
}
