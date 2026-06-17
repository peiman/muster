// OUT-005 exception: test skip-signal writes to stderr are legitimate test-harness
// communication (not library output). The Output struct cannot be used here.
#![allow(clippy::print_stderr)]
//! Hermetic consumer-layout fixture tests.
//!
//! These tests exercise the full arch_allowlist + violation_drift_guard behavior
//! matrix against minimal in-process workspace fixtures — no spawned processes,
//! no git operations, no network.  They simulate:
//!
//! 1. A consumer with custom layer names + config (enforced, pass and fail cases)
//! 2. A consumer with custom layout and NO config (loud skip)
//! 3. Scaffold layout with no config (strict defaults apply)
//! 4. Declared-but-missing layer (fail with clear message)
//! 5. violation_tests.enabled = false (drift guard skips)
//!
//! The loader logic itself is unit-tested in src/project_config.rs (injected
//! paths) — these fixture tests stay thin: they verify the end-to-end behavior
//! of the load/enforce pipeline.

use std::io::Write as _;

use ckeletin::project_config;
use ckeletin::project_config::{AllowlistsConfig, LoadOutcome};
use tempfile::TempDir;

// ── fixture helpers ───────────────────────────────────────────────────────────

/// Build a minimal workspace fixture on disk.
///
/// `files` is a list of (relative path, content) pairs.  Directories are
/// created automatically.
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

/// Build a minimal Cargo.toml for a crate with the given dependencies.
fn cargo_toml_with_deps(name: &str, deps: &[&str]) -> String {
    let mut s = format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n");
    if !deps.is_empty() {
        s.push_str("\n[dependencies]\n");
        for dep in deps {
            s.push_str(&format!("{dep} = \"1\"\n"));
        }
    }
    s
}

// ── enforce helpers (mirror what arch_allowlist.rs does, but in-process) ──────

/// Run the domain allowlist check against a fixture workspace.
/// Returns Ok(()) if the check passes, Err(String) with the failure message.
fn check_domain_allowlist(root: &std::path::Path) -> Result<(), String> {
    let outcome = project_config::load(root).map_err(|e| e.to_string())?;

    if outcome.should_skip() {
        return Err("SKIP".to_string());
    }

    let config = outcome.config();

    for domain_dir in &config.layers.domain {
        let cargo_toml = root.join(domain_dir).join("Cargo.toml");
        if !cargo_toml.exists() {
            return Err(format!(
                "Domain crate declared in ckeletin-project.toml [layers] not found: {}",
                cargo_toml.display()
            ));
        }

        let actual = parse_deps_from_toml(&cargo_toml)?;
        let allowed: std::collections::BTreeSet<String> = config
            .allowlists
            .domain
            .iter()
            .map(|s| s.to_string())
            .collect();

        let extra: Vec<_> = actual.difference(&allowed).cloned().collect();
        let missing: Vec<_> = allowed.difference(&actual).cloned().collect();

        if !extra.is_empty() {
            return Err(format!("extra deps in {domain_dir}: {extra:?}"));
        }
        if !missing.is_empty() {
            return Err(format!("missing deps in {domain_dir}: {missing:?}"));
        }
    }

    Ok(())
}

/// Run the infrastructure allowlist check against a fixture workspace.
fn check_infra_allowlist(root: &std::path::Path) -> Result<(), String> {
    let outcome = project_config::load(root).map_err(|e| e.to_string())?;

    if outcome.should_skip() {
        return Err("SKIP".to_string());
    }

    let config = outcome.config();

    for infra_dir in &config.layers.infrastructure {
        let cargo_toml = root.join(infra_dir).join("Cargo.toml");
        if !cargo_toml.exists() {
            return Err(format!(
                "Infrastructure crate declared in ckeletin-project.toml [layers] not found: {}",
                cargo_toml.display()
            ));
        }

        let actual = parse_deps_from_toml(&cargo_toml)?;
        let allowed: std::collections::BTreeSet<String> = config
            .allowlists
            .infrastructure
            .iter()
            .map(|s| s.to_string())
            .collect();

        let extra: Vec<_> = actual.difference(&allowed).cloned().collect();
        let missing: Vec<_> = allowed.difference(&actual).cloned().collect();

        if !extra.is_empty() {
            return Err(format!("extra deps in {infra_dir}: {extra:?}"));
        }
        if !missing.is_empty() {
            return Err(format!("missing deps in {infra_dir}: {missing:?}"));
        }
    }

    Ok(())
}

fn parse_deps_from_toml(
    path: &std::path::Path,
) -> Result<std::collections::BTreeSet<String>, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let parsed: toml::Value =
        toml::from_str(&content).map_err(|e| format!("cannot parse {}: {e}", path.display()))?;

    Ok(parsed
        .get("dependencies")
        .and_then(|d| d.as_table())
        .map(|table| table.keys().cloned().collect())
        .unwrap_or_default())
}

// ── Fixture 1a: custom layout + config → passes ───────────────────────────────

/// A consumer with custom layer names + ckeletin-project.toml that declares
/// exactly the right deps: check passes.
#[test]
fn consumer_custom_layout_with_config_passes_when_deps_match() {
    let ws = make_fixture(&[
        (
            "ckeletin-project.toml",
            r#"
[layers]
domain = ["chat-core"]
infrastructure = ["chat-infra"]
cli = ["chat-cli"]

[allowlists]
domain = ["serde"]
infrastructure = ["ckeletin"]
"#,
        ),
        (
            "chat-core/Cargo.toml",
            &cargo_toml_with_deps("chat-core", &["serde"]),
        ),
        (
            "chat-infra/Cargo.toml",
            &cargo_toml_with_deps("chat-infra", &["ckeletin"]),
        ),
    ]);

    let result = check_domain_allowlist(ws.path());
    assert!(
        result.is_ok(),
        "Expected pass but got: {:?}",
        result.unwrap_err()
    );

    let result = check_infra_allowlist(ws.path());
    assert!(
        result.is_ok(),
        "Expected pass but got: {:?}",
        result.unwrap_err()
    );
}

// ── Fixture 1b: custom layout + config → fails on extra dep ───────────────────

/// A consumer with custom layer names + config, but domain has an extra dep
/// not in the allowlist: check fails with clear message.
#[test]
fn consumer_custom_layout_with_config_fails_on_extra_dep() {
    let ws = make_fixture(&[
        (
            "ckeletin-project.toml",
            r#"
[layers]
domain = ["chat-core"]
infrastructure = ["chat-infra"]
cli = ["chat-cli"]

[allowlists]
domain = ["serde"]
infrastructure = ["ckeletin"]
"#,
        ),
        // chat-core has an extra dep (serde_json) not in the allowlist.
        (
            "chat-core/Cargo.toml",
            &cargo_toml_with_deps("chat-core", &["serde", "serde_json"]),
        ),
        (
            "chat-infra/Cargo.toml",
            &cargo_toml_with_deps("chat-infra", &["ckeletin"]),
        ),
    ]);

    let result = check_domain_allowlist(ws.path());
    assert!(result.is_err(), "Expected fail on extra dep, but got Ok");
    let msg = result.unwrap_err();
    assert!(
        msg.contains("serde_json"),
        "Error message should name the extra dep: {msg}"
    );
    assert!(
        msg.contains("extra deps"),
        "Error message should say 'extra deps': {msg}"
    );
}

// ── Fixture 1c: custom layout + config with extended allowlist → passes ────────

/// A consumer like workhorse with many justified domain deps declared in config.
#[test]
fn consumer_extended_domain_allowlist_passes() {
    let ws = make_fixture(&[
        (
            "ckeletin-project.toml",
            r#"
[layers]
domain = ["chat-core"]
infrastructure = ["chat-infra"]
cli = ["chat-cli"]

[allowlists]
# serde: typed serialization
# serde_json: JSON support
# thiserror: typed errors
domain = ["serde", "serde_json", "thiserror"]
infrastructure = ["ckeletin"]
"#,
        ),
        (
            "chat-core/Cargo.toml",
            &cargo_toml_with_deps("chat-core", &["serde", "serde_json", "thiserror"]),
        ),
        (
            "chat-infra/Cargo.toml",
            &cargo_toml_with_deps("chat-infra", &["ckeletin"]),
        ),
    ]);

    let result = check_domain_allowlist(ws.path());
    assert!(
        result.is_ok(),
        "Expected pass with extended allowlist, got: {:?}",
        result.unwrap_err()
    );
}

// ── Fixture 2: custom layout + NO config → loud skip ─────────────────────────

/// A consumer with a custom layout and NO ckeletin-project.toml.
/// The scaffold crates/domain/ does NOT exist.
/// Result: LoadOutcome::Absent → skip loudly (not a panic, not a fail).
#[test]
fn consumer_custom_layout_no_config_returns_absent() {
    // Custom layout only — no crates/domain.
    let ws = make_fixture(&[(
        "chat-core/Cargo.toml",
        &cargo_toml_with_deps("chat-core", &["serde"]),
    )]);

    let outcome = project_config::load(ws.path()).unwrap();
    assert!(
        outcome.should_skip(),
        "Expected Absent (should_skip=true) for custom layout with no config"
    );
    assert!(
        matches!(outcome, LoadOutcome::Absent),
        "Expected LoadOutcome::Absent, got something else"
    );
}

// ── Fixture 3: scaffold layout + NO config → scaffold defaults ────────────────

/// Standard scaffold layout (crates/domain/Cargo.toml exists) with no
/// ckeletin-project.toml: scaffold defaults apply, check passes when deps match.
#[test]
fn scaffold_layout_no_config_uses_defaults_and_passes() {
    let ws = make_fixture(&[
        (
            "crates/domain/Cargo.toml",
            &cargo_toml_with_deps("domain", &["serde"]),
        ),
        (
            "crates/infrastructure/Cargo.toml",
            &cargo_toml_with_deps("infrastructure", &["ckeletin"]),
        ),
    ]);

    let outcome = project_config::load(ws.path()).unwrap();
    assert!(
        matches!(outcome, LoadOutcome::ScaffoldDefaults(_)),
        "Expected ScaffoldDefaults for scaffold layout with no config"
    );

    let result = check_domain_allowlist(ws.path());
    assert!(
        result.is_ok(),
        "Expected pass with scaffold defaults, got: {:?}",
        result.unwrap_err()
    );

    let result = check_infra_allowlist(ws.path());
    assert!(
        result.is_ok(),
        "Expected pass with scaffold defaults, got: {:?}",
        result.unwrap_err()
    );
}

/// Scaffold layout with no config but a domain dep outside the default
/// allowlist: check fails with a clear message pointing to ckeletin-project.toml.
#[test]
fn scaffold_layout_no_config_fails_on_extra_dep() {
    let ws = make_fixture(&[
        // Domain has serde_json — not in the scaffold default allowlist.
        (
            "crates/domain/Cargo.toml",
            &cargo_toml_with_deps("domain", &["serde", "serde_json"]),
        ),
    ]);

    let result = check_domain_allowlist(ws.path());
    assert!(result.is_err(), "Expected fail on extra dep");
    let msg = result.unwrap_err();
    assert!(
        msg.contains("serde_json"),
        "Error should name the dep: {msg}"
    );
}

// ── Fixture 4: declared-but-missing layer → fail with clear message ───────────

/// A consumer declares a domain crate in ckeletin-project.toml but the
/// directory doesn't exist: check fails with a clear diagnostic.
#[test]
fn declared_layer_missing_on_disk_fails_clearly() {
    let ws = make_fixture(&[(
        "ckeletin-project.toml",
        r#"
[layers]
domain = ["nonexistent-core"]
infrastructure = ["chat-infra"]
cli = ["chat-cli"]

[allowlists]
domain = ["serde"]
infrastructure = ["ckeletin"]
"#,
    )]);

    let result = check_domain_allowlist(ws.path());
    assert!(
        result.is_err(),
        "Expected fail when declared layer is missing from disk"
    );
    let msg = result.unwrap_err();
    assert!(
        msg.contains("nonexistent-core"),
        "Error should name the missing crate path: {msg}"
    );
    // Must not say "scaffold" — this is about the declared path being wrong.
    assert!(
        !msg.contains("scaffold"),
        "Error should not mention scaffold: {msg}"
    );
}

// ── Fixture 5: violation_tests.enabled = false → drift guard skips ────────────

/// When violation_tests.enabled = false, the drift guard reports it should
/// skip (the config flag is reflected in the loaded config).
#[test]
fn violation_tests_disabled_flag_is_honoured() {
    let ws = make_fixture(&[(
        "ckeletin-project.toml",
        r#"
[layers]
domain = ["ioguard-core"]
infrastructure = ["ioguard-infra"]
cli = ["ioguard-cli"]

[violation_tests]
# ioguard does not ship trybuild violation test copies; the boundary is
# enforced at compile time via Cargo.toml alone.
enabled = false
"#,
    )]);

    let outcome = project_config::load(ws.path()).unwrap();
    assert!(matches!(outcome, LoadOutcome::Explicit(_)));
    let config = outcome.config();
    assert!(
        !config.violation_tests.enabled,
        "violation_tests.enabled should be false"
    );
}

// ── Fixture 6: multi-crate domain → all checked ───────────────────────────────

/// A consumer with multiple domain crates (agent-chat shape: chat-core +
/// chat-models). All must satisfy the allowlist.
#[test]
fn multi_crate_domain_all_checked() {
    let ws = make_fixture(&[
        (
            "ckeletin-project.toml",
            r#"
[layers]
domain = ["chat-core", "chat-models"]
infrastructure = ["chat-infra"]
cli = ["chat-cli"]

[allowlists]
domain = ["serde"]
infrastructure = ["ckeletin"]
"#,
        ),
        (
            "chat-core/Cargo.toml",
            &cargo_toml_with_deps("chat-core", &["serde"]),
        ),
        (
            "chat-models/Cargo.toml",
            &cargo_toml_with_deps("chat-models", &["serde"]),
        ),
        (
            "chat-infra/Cargo.toml",
            &cargo_toml_with_deps("chat-infra", &["ckeletin"]),
        ),
    ]);

    let result = check_domain_allowlist(ws.path());
    assert!(
        result.is_ok(),
        "Expected pass for multi-crate domain, got: {:?}",
        result.unwrap_err()
    );
}

/// Same as above but the second domain crate has a forbidden dep: FAIL.
#[test]
fn multi_crate_domain_second_crate_violation_detected() {
    let ws = make_fixture(&[
        (
            "ckeletin-project.toml",
            r#"
[layers]
domain = ["chat-core", "chat-models"]
infrastructure = ["chat-infra"]
cli = ["chat-cli"]

[allowlists]
domain = ["serde"]
infrastructure = ["ckeletin"]
"#,
        ),
        (
            "chat-core/Cargo.toml",
            &cargo_toml_with_deps("chat-core", &["serde"]),
        ),
        // chat-models sneaks in an extra dep.
        (
            "chat-models/Cargo.toml",
            &cargo_toml_with_deps("chat-models", &["serde", "uuid"]),
        ),
        (
            "chat-infra/Cargo.toml",
            &cargo_toml_with_deps("chat-infra", &["ckeletin"]),
        ),
    ]);

    let result = check_domain_allowlist(ws.path());
    assert!(result.is_err(), "Expected fail on second crate violation");
    let msg = result.unwrap_err();
    assert!(
        msg.contains("uuid"),
        "Error should name the forbidden dep: {msg}"
    );
}

// ── Fixture 7: allowlists defaults apply when [allowlists] omitted ────────────

/// Config with [layers] only — [allowlists] defaults to domain=["serde"],
/// infra=["ckeletin"].
#[test]
fn allowlist_defaults_apply_when_section_omitted() {
    let ws = make_fixture(&[
        (
            "ckeletin-project.toml",
            r#"
[layers]
domain = ["chat-core"]
infrastructure = ["chat-infra"]
cli = ["chat-cli"]
"#,
        ),
        (
            "chat-core/Cargo.toml",
            &cargo_toml_with_deps("chat-core", &["serde"]),
        ),
        (
            "chat-infra/Cargo.toml",
            &cargo_toml_with_deps("chat-infra", &["ckeletin"]),
        ),
    ]);

    let outcome = project_config::load(ws.path()).unwrap();
    let config = outcome.config();
    assert_eq!(
        config.allowlists,
        AllowlistsConfig::default(),
        "allowlists should default to scaffold values when section is omitted"
    );
    let result = check_domain_allowlist(ws.path());
    assert!(
        result.is_ok(),
        "Expected pass with default allowlists, got: {:?}",
        result.unwrap_err()
    );
}

#[test]
fn empty_layer_list_is_an_honest_declaration_not_an_error() {
    // The agent-chat / ioguard shape (consumer feedback, 2026-06-10): adapter
    // crates (chat-daemon, ioguard-ffi) import the core BY DESIGN, which the
    // ckeletin infrastructure rules forbid — so those repos declare
    // `infrastructure = []` rather than a forced or false mapping. An empty
    // layer list must mean "this architecture has no such layer: enforce
    // nothing for it", while the layers that ARE declared stay enforced.
    // Two real consumers depend on this; a future refactor that turns an
    // empty list into an error or a panic breaks both.
    let ws = make_fixture(&[
        (
            "ckeletin-project.toml",
            r#"
[layers]
domain = ["chat-core"]
infrastructure = []
cli = ["chat-cli"]

[allowlists]
domain = ["serde"]
"#,
        ),
        (
            "chat-core/Cargo.toml",
            &cargo_toml_with_deps("chat-core", &["serde"]),
        ),
        // The adapter crate imports the core — fine, because no
        // infrastructure layer is declared for it to violate.
        (
            "chat-daemon/Cargo.toml",
            &cargo_toml_with_deps("chat-daemon", &["chat-core", "tokio"]),
        ),
    ]);

    let infra = check_infra_allowlist(ws.path());
    assert!(
        infra.is_ok(),
        "empty infrastructure layer list must enforce nothing, got: {:?}",
        infra.unwrap_err()
    );

    // Declared layers stay enforced in the same config: a domain violation
    // is still caught.
    let domain = check_domain_allowlist(ws.path());
    assert!(domain.is_ok(), "domain within allowlist must pass");
    std::fs::write(
        ws.path().join("chat-core/Cargo.toml"),
        cargo_toml_with_deps("chat-core", &["serde", "reqwest"]),
    )
    .unwrap();
    let domain_violation = check_domain_allowlist(ws.path());
    assert!(
        domain_violation.is_err(),
        "declared domain layer must still be enforced alongside empty layers"
    );
}
