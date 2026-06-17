// OUT-005 exception: no stdout/stderr output from this module.
//! Project-owned conformance configuration loader.
//!
//! `ckeletin-project.toml` at the repo root is the **project-owned** file where
//! consumers declare:
//! - The paths of their layer crates (`[layers]`)
//! - Per-layer dependency allowlists (`[allowlists]`)
//! - Where violation test copies live (`[violation_tests]`)
//!
//! This file survives `just ckeletin-update` (it lives outside `.ckeletin/`).
//! It is the SSOT for everything a consumer needs to tailor — framework-owned
//! test files (`.ckeletin/crate/tests/`) read from it and must never be edited.
//!
//! ## Loading behaviour
//!
//! | Condition | Behaviour |
//! |-----------|-----------|
//! | File present and valid | Use declared config strictly |
//! | File absent, scaffold layout present (`crates/domain` exists) | Use scaffold defaults |
//! | File absent, scaffold layout absent | Return `ProjectConfig::absent()` — callers skip with a nudge |
//! | File present but malformed | Return `Err` — loud parse error |

use std::path::{Path, PathBuf};

use figment::{
    Figment,
    providers::{Format, Toml},
};
use serde::{Deserialize, Serialize};

/// File name of the project-owned config at the workspace root.
pub const PROJECT_CONFIG_FILE: &str = "ckeletin-project.toml";

/// Scaffold-default layer paths (what the unmodified scaffold ships with).
pub const SCAFFOLD_DOMAIN: &str = "crates/domain";
pub const SCAFFOLD_INFRASTRUCTURE: &str = "crates/infrastructure";
pub const SCAFFOLD_CLI: &str = "crates/cli";

// ── Schema ────────────────────────────────────────────────────────────────────

/// Project-owned conformance configuration.
///
/// Schema: https://github.com/peiman/ckeletin (ckeletin-project.toml)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectConfig {
    /// Layer crate paths.
    #[serde(default)]
    pub layers: LayersConfig,

    /// Per-layer dependency allowlists.
    #[serde(default)]
    pub allowlists: AllowlistsConfig,

    /// Violation test configuration.
    #[serde(default)]
    pub violation_tests: ViolationTestsConfig,
}

/// Layer crate paths.
///
/// Each field is a list of paths so consumers with multiple domain-layer crates
/// (e.g. `chat-core`, `chat-daemon`) can declare all of them.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct LayersConfig {
    /// Domain crate path(s). Must not contain framework deps.
    #[serde(default)]
    pub domain: Vec<String>,

    /// Infrastructure crate path(s). Must not import domain/cli.
    #[serde(default)]
    pub infrastructure: Vec<String>,

    /// CLI crate path(s). The only crate allowed to use clap.
    #[serde(default)]
    pub cli: Vec<String>,
}

/// Per-layer dependency allowlists.
///
/// These are the complete `[dependencies]` sections allowed for each layer.
/// Adding a dep to a layer requires adding it here — which is the point.
///
/// **TOML comments are the justification mechanism.** Above each entry, add
/// a comment explaining why the dep belongs there.  Example:
///
/// ```toml
/// [allowlists]
/// # serde: typed serialization for domain result types (CKSPEC-ARCH-004)
/// # serde_json: JSON serialization for domain result types
/// domain = ["serde", "serde_json"]
/// infrastructure = ["ckeletin"]
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AllowlistsConfig {
    /// Allowed deps for domain crate(s). Default: `["serde"]`.
    #[serde(default = "defaults::domain_allowlist")]
    pub domain: Vec<String>,

    /// Allowed deps for infrastructure crate(s). Default: `["ckeletin"]`.
    #[serde(default = "defaults::infra_allowlist")]
    pub infrastructure: Vec<String>,
}

impl Default for AllowlistsConfig {
    fn default() -> Self {
        Self {
            domain: defaults::domain_allowlist(),
            infrastructure: defaults::infra_allowlist(),
        }
    }
}

/// Violation test configuration.
///
/// Declares where the project copies of violation test files live so the
/// drift guard knows what to compare against the vendored `.ckeletin/` copies.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ViolationTestsConfig {
    /// Whether violation tests are active in this project.
    /// Set to `false` with a reason comment if this layout doesn't carry them.
    #[serde(default = "defaults::violation_tests_enabled")]
    pub enabled: bool,

    /// Additional domain violation test directories (beyond the default
    /// `crates/domain/tests/violations/`). Only needed for non-scaffold layouts.
    #[serde(default)]
    pub domain_dirs: Vec<String>,

    /// Additional infra violation test directories (beyond the default
    /// `crates/infrastructure/tests/violations/`). Only needed for non-scaffold layouts.
    #[serde(default)]
    pub infra_dirs: Vec<String>,
}

impl Default for ViolationTestsConfig {
    fn default() -> Self {
        Self {
            enabled: defaults::violation_tests_enabled(),
            domain_dirs: vec![],
            infra_dirs: vec![],
        }
    }
}

mod defaults {
    pub fn domain_allowlist() -> Vec<String> {
        vec!["serde".to_string()]
    }
    pub fn infra_allowlist() -> Vec<String> {
        vec!["ckeletin".to_string()]
    }
    pub fn violation_tests_enabled() -> bool {
        true
    }
}

// ── Load outcome ─────────────────────────────────────────────────────────────

/// Outcome of attempting to load `ckeletin-project.toml`.
#[derive(Debug, Clone, PartialEq)]
pub enum LoadOutcome {
    /// Config file was present and parsed successfully.
    Explicit(ProjectConfig),
    /// Config file was absent but scaffold layout detected — strict defaults apply.
    ScaffoldDefaults(ProjectConfig),
    /// Config file was absent and scaffold layout absent — tests should skip loudly.
    Absent,
}

impl LoadOutcome {
    /// True when this outcome means "skip the test loudly".
    pub fn should_skip(&self) -> bool {
        matches!(self, LoadOutcome::Absent)
    }

    /// Extract the config, panicking if absent (caller checked `should_skip` first).
    pub fn config(&self) -> &ProjectConfig {
        match self {
            LoadOutcome::Explicit(c) | LoadOutcome::ScaffoldDefaults(c) => c,
            LoadOutcome::Absent => panic!("called config() on an Absent LoadOutcome"),
        }
    }
}

// ── Loader ───────────────────────────────────────────────────────────────────

/// Load `ckeletin-project.toml` relative to `workspace_root`.
///
/// Returns:
/// - `Ok(LoadOutcome::Explicit(_))` — file found and parsed.
/// - `Ok(LoadOutcome::ScaffoldDefaults(_))` — file absent, scaffold layout present.
/// - `Ok(LoadOutcome::Absent)` — file absent, no scaffold layout detected.
/// - `Err(_)` — file present but malformed (loud parse error, never silenced).
pub fn load(workspace_root: &Path) -> Result<LoadOutcome, Box<dyn std::error::Error>> {
    let config_path = workspace_root.join(PROJECT_CONFIG_FILE);

    if config_path.exists() {
        // File present: parse strictly. Malformed = loud error (never silenced).
        let config: ProjectConfig = Figment::new()
            .merge(Toml::file(&config_path))
            .extract()
            .map_err(|e| {
                format!(
                    "Failed to parse {}: {}\n\
                     Fix the TOML syntax or field values in ckeletin-project.toml.",
                    config_path.display(),
                    e
                )
            })?;
        return Ok(LoadOutcome::Explicit(config));
    }

    // No config file — detect scaffold layout.
    if scaffold_layout_present(workspace_root) {
        let config = scaffold_defaults();
        return Ok(LoadOutcome::ScaffoldDefaults(config));
    }

    Ok(LoadOutcome::Absent)
}

/// True when the scaffold layer crates all exist under `workspace_root`.
fn scaffold_layout_present(workspace_root: &Path) -> bool {
    let domain_toml = workspace_root.join(SCAFFOLD_DOMAIN).join("Cargo.toml");
    domain_toml.exists()
}

/// Build a `ProjectConfig` representing the scaffold's default layout.
pub fn scaffold_defaults() -> ProjectConfig {
    ProjectConfig {
        layers: LayersConfig {
            domain: vec![SCAFFOLD_DOMAIN.to_string()],
            infrastructure: vec![SCAFFOLD_INFRASTRUCTURE.to_string()],
            cli: vec![SCAFFOLD_CLI.to_string()],
        },
        allowlists: AllowlistsConfig::default(),
        violation_tests: ViolationTestsConfig::default(),
    }
}

/// Resolve a layer's Cargo.toml paths given crate directory paths.
pub fn cargo_toml_paths(workspace_root: &Path, dirs: &[String]) -> Vec<PathBuf> {
    dirs.iter()
        .map(|d| workspace_root.join(d).join("Cargo.toml"))
        .collect()
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;
    use tempfile::TempDir;

    fn make_workspace(files: &[(&str, &str)]) -> TempDir {
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

    // ── parse tests ──────────────────────────────────────────────────────────

    #[test]
    fn load_explicit_minimal_config() {
        // Minimal config: just layers, allowlists get defaults.
        let ws = make_workspace(&[(
            "ckeletin-project.toml",
            r#"
[layers]
domain = ["chat-core"]
infrastructure = ["chat-infra"]
cli = ["chat-cli"]
"#,
        )]);
        let outcome = load(ws.path()).unwrap();
        assert!(matches!(outcome, LoadOutcome::Explicit(_)));
        let config = outcome.config();
        assert_eq!(config.layers.domain, vec!["chat-core"]);
        assert_eq!(config.layers.infrastructure, vec!["chat-infra"]);
        assert_eq!(config.layers.cli, vec!["chat-cli"]);
        // Allowlist defaults apply.
        assert_eq!(config.allowlists.domain, vec!["serde"]);
        assert_eq!(config.allowlists.infrastructure, vec!["ckeletin"]);
    }

    #[test]
    fn load_explicit_with_custom_allowlists() {
        let ws = make_workspace(&[(
            "ckeletin-project.toml",
            r#"
[layers]
domain = ["chat-core"]
infrastructure = ["chat-infra"]
cli = ["chat-cli"]

[allowlists]
# serde: typed serialization
# serde_json: JSON support
domain = ["serde", "serde_json"]
infrastructure = ["ckeletin"]
"#,
        )]);
        let outcome = load(ws.path()).unwrap();
        assert!(matches!(outcome, LoadOutcome::Explicit(_)));
        let config = outcome.config();
        assert_eq!(config.allowlists.domain, vec!["serde", "serde_json"]);
    }

    #[test]
    fn load_absent_with_scaffold_layout_returns_scaffold_defaults() {
        // No config file, but scaffold crates/domain/Cargo.toml exists.
        let ws = make_workspace(&[("crates/domain/Cargo.toml", "[package]\nname=\"domain\"\n")]);
        let outcome = load(ws.path()).unwrap();
        assert!(matches!(outcome, LoadOutcome::ScaffoldDefaults(_)));
        let config = outcome.config();
        assert_eq!(config.layers.domain, vec![SCAFFOLD_DOMAIN]);
        assert_eq!(config.layers.infrastructure, vec![SCAFFOLD_INFRASTRUCTURE]);
        assert_eq!(config.layers.cli, vec![SCAFFOLD_CLI]);
        assert_eq!(config.allowlists.domain, vec!["serde"]);
    }

    #[test]
    fn load_absent_without_scaffold_returns_absent() {
        // No config file, no scaffold layout.
        let ws = make_workspace(&[]);
        let outcome = load(ws.path()).unwrap();
        assert!(matches!(outcome, LoadOutcome::Absent));
        assert!(outcome.should_skip());
    }

    #[test]
    fn load_malformed_toml_returns_error() {
        let ws = make_workspace(&[("ckeletin-project.toml", "[layers\nbad toml [[[")]);
        let result = load(ws.path());
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("ckeletin-project.toml"),
            "Error message should name the file: {msg}"
        );
    }

    #[test]
    fn load_empty_config_uses_all_defaults() {
        // An empty file still parses (all fields have defaults).
        // But layers fields have no defaults — empty vecs imply no layers declared.
        let ws = make_workspace(&[("ckeletin-project.toml", "")]);
        let outcome = load(ws.path()).unwrap();
        assert!(matches!(outcome, LoadOutcome::Explicit(_)));
        let config = outcome.config();
        // Empty layers = no crates declared.
        assert!(config.layers.domain.is_empty());
        assert_eq!(config.allowlists.domain, vec!["serde"]);
        assert!(config.violation_tests.enabled);
    }

    #[test]
    fn load_multi_crate_domain() {
        let ws = make_workspace(&[(
            "ckeletin-project.toml",
            r#"
[layers]
domain = ["chat-core", "chat-models"]
infrastructure = ["chat-infra"]
cli = ["chat-cli"]
"#,
        )]);
        let outcome = load(ws.path()).unwrap();
        let config = outcome.config();
        assert_eq!(config.layers.domain, vec!["chat-core", "chat-models"]);
    }

    #[test]
    fn load_violation_tests_disabled() {
        let ws = make_workspace(&[(
            "ckeletin-project.toml",
            r#"
[layers]
domain = ["ioguard-core"]
infrastructure = ["ioguard-infra"]
cli = ["ioguard-cli"]

[violation_tests]
enabled = false
"#,
        )]);
        let outcome = load(ws.path()).unwrap();
        let config = outcome.config();
        assert!(!config.violation_tests.enabled);
    }

    // ── scaffold_defaults tests ───────────────────────────────────────────────

    #[test]
    fn scaffold_defaults_match_expected_paths() {
        let config = scaffold_defaults();
        assert_eq!(config.layers.domain, vec!["crates/domain"]);
        assert_eq!(config.layers.infrastructure, vec!["crates/infrastructure"]);
        assert_eq!(config.layers.cli, vec!["crates/cli"]);
    }

    // ── cargo_toml_paths tests ────────────────────────────────────────────────

    #[test]
    fn cargo_toml_paths_resolves_relative_to_root() {
        let ws = make_workspace(&[]);
        let paths = cargo_toml_paths(ws.path(), &["crates/domain".to_string()]);
        assert_eq!(paths.len(), 1);
        assert!(paths[0].ends_with("crates/domain/Cargo.toml"));
    }
}
