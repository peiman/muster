use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::process::Command;

// ── Spec requirements source ──────────────────────────────────

const REQUIREMENTS_JSON_URL: &str =
    "https://raw.githubusercontent.com/peiman/ckeletin/main/spec/requirements.json";
const VENDORED_REQUIREMENTS: &str = "conformance/requirements.json";
const PUBLISHED_REPORT: &str = "conformance-report.json";
const CONFORMANCE_MD: &str = "CONFORMANCE.md";

#[derive(Deserialize)]
struct SpecManifest {
    spec_version: String,
    requirements: Vec<SpecRequirement>,
}

#[derive(Deserialize)]
struct SpecRequirement {
    id: String,
}

// ── Mapping file types (read from TOML) ─────────────────────────

#[derive(Deserialize)]
struct Mapping {
    spec_version: String,
    requirements: BTreeMap<String, RequirementMapping>,
}

#[derive(Deserialize, Default)]
struct RequirementMapping {
    title: String,
    status: String,
    enforcement_level: String,
    evidence: String,
    #[serde(default)]
    checks: Vec<String>,
    #[serde(default)]
    violation_tests: Vec<String>,
    #[serde(default)]
    violation_evidence: Option<String>,
}

// ── Report types (output as JSON) ───────────────────────────────

#[derive(Serialize)]
struct Report {
    implementation: String,
    spec_version: String,
    report_date: String,
    summary: Summary,
    requirements: BTreeMap<String, RequirementResult>,
    feedback: Vec<String>,
}

#[derive(Serialize)]
struct Summary {
    total: usize,
    met: usize,
    partial: usize,
    deferred: usize,
    failed_checks: usize,
    feedback_signals: usize,
}

#[derive(Serialize)]
struct RequirementResult {
    title: String,
    status: String,
    enforcement_level: String,
    evidence: String,
    checks: Vec<CheckResult>,
    violation_tests: Vec<ViolationTestResult>,
}

#[derive(Serialize)]
struct CheckResult {
    command: String,
    passed: bool,
}

#[derive(Serialize)]
struct ViolationTestResult {
    path: String,
    exists: bool,
}

// ── Requirement ID loading (replaces hardcoded list) ────────────

/// Load the spec requirement IDs.
///
/// Default (CI / gating): read ONLY the committed vendored requirements.json —
/// offline, deterministic, and side-effect-free. The conformance gate must not
/// depend on a moving upstream branch (a push to a *different* repo could
/// otherwise turn this repo's CI red) nor mutate a tracked file mid-run.
///
/// With `refresh = true` (`conform --refresh` / `just conform-refresh`): fetch
/// the latest requirements from the spec repo and rewrite the vendored copy, so
/// a maintainer can review the diff and reconcile conformance-mapping.toml
/// deliberately — turning a spec bump into an intentional, reviewed commit.
fn load_spec_requirements(refresh: bool, json_mode: bool) -> (Vec<String>, String) {
    if refresh {
        match fetch_upstream() {
            Ok(manifest) => {
                let json = serde_json::to_string_pretty(&serde_json::json!({
                    "spec_version": manifest.spec_version,
                    "requirements": manifest.requirements.iter().map(|r| {
                        serde_json::json!({"id": r.id})
                    }).collect::<Vec<_>>()
                }))
                .expect("serialize requirements");
                if let Err(e) = std::fs::write(VENDORED_REQUIREMENTS, format!("{json}\n")) {
                    eprintln!(
                        "Error: fetched spec but could not write {VENDORED_REQUIREMENTS}: {e}"
                    );
                    std::process::exit(1);
                }
                if !json_mode {
                    eprintln!(
                        "Refreshed {VENDORED_REQUIREMENTS} from upstream (spec {}). Review the diff and reconcile conformance-mapping.toml.",
                        manifest.spec_version
                    );
                }
                let ids = manifest.requirements.iter().map(|r| r.id.clone()).collect();
                (ids, manifest.spec_version)
            }
            Err(fetch_err) => {
                let msg = format!(
                    "--refresh requested but could not fetch upstream spec: {fetch_err} (URL: {REQUIREMENTS_JSON_URL})"
                );
                if json_mode {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(
                            &serde_json::json!({ "status": "error", "error": msg })
                        )
                        .unwrap()
                    );
                } else {
                    eprintln!("Error: {msg}");
                }
                std::process::exit(1);
            }
        }
    } else {
        match load_vendored() {
            Ok(manifest) => {
                let ids = manifest.requirements.iter().map(|r| r.id.clone()).collect();
                (ids, manifest.spec_version)
            }
            Err(vendor_err) => {
                let msg = format!(
                    "cannot read vendored spec {VENDORED_REQUIREMENTS}: {vendor_err}. Run `cargo run -p ckeletin-conform -- --refresh` (or `just conform-refresh`) to fetch it."
                );
                if json_mode {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(
                            &serde_json::json!({ "status": "error", "error": msg })
                        )
                        .unwrap()
                    );
                } else {
                    eprintln!("Error: {msg}");
                }
                std::process::exit(1);
            }
        }
    }
}

fn fetch_upstream() -> Result<SpecManifest, String> {
    let body: Vec<u8> = ureq::get(REQUIREMENTS_JSON_URL)
        .call()
        .map_err(|e| format!("{e}"))?
        .body_mut()
        .read_to_vec()
        .map_err(|e| format!("{e}"))?;
    serde_json::from_slice(&body).map_err(|e| format!("parse error: {e}"))
}

fn load_vendored() -> Result<SpecManifest, String> {
    let content = std::fs::read_to_string(VENDORED_REQUIREMENTS).map_err(|e| format!("{e}"))?;
    serde_json::from_str(&content).map_err(|e| format!("parse error: {e}"))
}

/// CKSPEC-ENF-005: requirement IDs present in the spec but absent from the
/// mapping. A non-empty result is a completeness violation and aborts the run.
fn find_unmapped(
    expected_ids: &[String],
    mapping: &BTreeMap<String, RequirementMapping>,
) -> Vec<String> {
    expected_ids
        .iter()
        .filter(|id| !mapping.contains_key(id.as_str()))
        .cloned()
        .collect()
}

/// CKSPEC-ENF-005 (reverse): requirement IDs present in the mapping but absent
/// from the spec. Extra entries inflate totals and indicate a stale or invented
/// requirement ID — a hard failure, not a silent pass.
fn find_extra(
    expected_ids: &[String],
    mapping: &BTreeMap<String, RequirementMapping>,
) -> Vec<String> {
    mapping
        .keys()
        .filter(|id| !expected_ids.contains(id))
        .cloned()
        .collect()
}

/// CKSPEC-ENF-006: an enforcement claim above honor-system/design MUST carry a
/// violation test or violation_evidence. Returns true when that proof is
/// missing (which the generator surfaces as an ENF-007 feedback signal).
fn lacks_enforcement_proof(req: &RequirementMapping) -> bool {
    let above_honor = !matches!(req.enforcement_level.as_str(), "honor-system" | "design");
    let has_violation_test = !req.violation_tests.is_empty();
    let has_violation_evidence = req
        .violation_evidence
        .as_ref()
        .is_some_and(|e| !e.is_empty());
    above_honor && !has_violation_test && !has_violation_evidence
}

/// CKSPEC-ENF-008: a `met` requirement MUST be anchored to verifiable evidence —
/// at least one of an automated check, a violation test, or written
/// violation_evidence. Returns true when a met claim has none (which fails the
/// conform gate so an unanchored claim can't be published).
fn lacks_anchor(req: &RequirementMapping) -> bool {
    req.status == "met"
        && req.checks.is_empty()
        && req.violation_tests.is_empty()
        && req
            .violation_evidence
            .as_deref()
            .unwrap_or("")
            .trim()
            .is_empty()
}

/// Dangling anchor check: for every evidence string that names a file path
/// (contains `/` and ends in a known source extension or is a plain path),
/// verify the file exists. For paths containing `::` (e.g. `file.rs::fn_name`),
/// also verify the symbol name appears in the file. Returns a list of
/// (requirement_id, problem_description) pairs for dangling anchors.
///
/// Heuristics used to identify file-path anchors:
/// - The token contains a `/`
/// - The token ends in `.rs`, `.toml`, `.yaml`, `.yml`, `.json`, `.md`, or `.txt`
/// - The token does NOT start with `http`
/// - The token starts with a word char (not a sentence fragment)
fn find_dangling_anchors(mapping: &BTreeMap<String, RequirementMapping>) -> Vec<(String, String)> {
    let mut dangling = Vec::new();

    for (req_id, req) in mapping {
        let anchors = collect_path_anchors(req);
        for anchor in anchors {
            let (file_part, symbol_part) = split_anchor(&anchor);
            let path = std::path::Path::new(file_part);
            if !path.exists() {
                dangling.push((
                    req_id.clone(),
                    format!("evidence anchor path not found: {file_part}"),
                ));
            } else if let Some(symbol) = symbol_part {
                // File exists — verify the symbol appears in it.
                match std::fs::read_to_string(path) {
                    Ok(content) => {
                        if !content.contains(symbol) {
                            dangling.push((
                                req_id.clone(),
                                format!(
                                    "evidence anchor symbol `{symbol}` not found in {file_part}"
                                ),
                            ));
                        }
                    }
                    Err(e) => {
                        dangling.push((
                            req_id.clone(),
                            format!("evidence anchor file {file_part} unreadable: {e}"),
                        ));
                    }
                }
            }
        }
    }

    dangling
}

/// Collect all file-path-shaped tokens from evidence fields of a requirement.
fn collect_path_anchors(req: &RequirementMapping) -> Vec<String> {
    let mut anchors = Vec::new();
    for source in [
        req.evidence.as_str(),
        req.violation_evidence.as_deref().unwrap_or(""),
    ] {
        for token in tokenize_anchors(source) {
            anchors.push(token);
        }
    }
    for vt in &req.violation_tests {
        anchors.push(vt.clone());
    }
    anchors
}

/// Extract file-path tokens from a freeform text string.
/// A token is considered a file path when it:
///   - contains `/`
///   - ends in `.rs`, `.toml`, `.yaml`, `.yml`, `.json`, `.md`, `.txt`
///     OR is of the form `<path>::<symbol>` where the path part ends in one
///     of those extensions (e.g. `crates/domain/src/logging.rs::validate_level`)
///   - does NOT start with `http`
fn tokenize_anchors(text: &str) -> Vec<String> {
    let extensions = [".rs", ".toml", ".yaml", ".yml", ".json", ".md", ".txt"];
    let mut tokens = Vec::new();
    // Split on whitespace and common punctuation that wouldn't be in a path.
    for raw in text.split(|c: char| {
        c.is_whitespace() || matches!(c, ',' | ';' | '(' | ')' | '\'' | '"' | '[' | ']')
    }) {
        // Strip trailing punctuation only — preserve leading dots (e.g. `.ckeletin/`
        // is a valid path prefix and must not be stripped to `ckeletin/`).
        let token = raw
            .trim_end_matches(['.', ':', '`'])
            .trim_start_matches('`');
        if token.starts_with("http") {
            continue;
        }
        if !token.contains('/') {
            continue;
        }
        // Plain path: token itself ends with a known extension.
        if extensions.iter().any(|ext| token.ends_with(ext)) {
            tokens.push(token.to_string());
            continue;
        }
        // Symbol reference: `<path>::<symbol>` — check the path part.
        if let Some(pos) = token.rfind("::") {
            let path_part = &token[..pos];
            if extensions.iter().any(|ext| path_part.ends_with(ext)) {
                tokens.push(token.to_string());
            }
        }
    }
    tokens
}

/// Split `file.rs::symbol` into (`file.rs`, Some(`symbol`)).
/// Returns (`anchor`, None) when there is no `::` separator.
fn split_anchor(anchor: &str) -> (&str, Option<&str>) {
    if let Some(pos) = anchor.rfind("::") {
        let file = &anchor[..pos];
        // Only treat it as a symbol reference when the file part ends in .rs
        if file.ends_with(".rs") {
            return (file, Some(&anchor[pos + 2..]));
        }
    }
    (anchor, None)
}

// ── Published report (CKSPEC-ENF-010) ───────────────────────────
// A deterministic projection of conformance-mapping.toml. Field order is
// alphabetical (matching ckeletin-go's report) and there is NO timestamp, so the
// committed report is byte-stable and sync-checkable; the spec-repo aggregator
// stamps the fetch date.

#[derive(Serialize)]
struct PublishedReport {
    implementation: String,
    requirements: BTreeMap<String, PublishedRequirement>,
    spec_version: String,
    summary: PublishedSummary,
}

#[derive(Serialize)]
struct PublishedRequirement {
    checks: Vec<String>,
    enforcement_level: String,
    evidence: String,
    status: String,
    violation_evidence: Option<String>,
    violation_tests: Vec<String>,
}

#[derive(Serialize)]
struct PublishedSummary {
    deferred: usize,
    met: usize,
    partial: usize,
    /// True when no requirement is declared `partial` or `deferred` — i.e. the
    /// mapping *claims* full conformance. This reflects declared STATUS only,
    /// not runtime check results: the report is projected before the mapped
    /// checks run, so `conform` itself is what gates a green tree (it exits
    /// non-zero on a failed check or an unanchored `met`). The report is only
    /// committed via `just conform-report`, which a maintainer runs on a tree
    /// that already passes `just conform`. Field name mirrors ckeletin-go's
    /// report schema.
    passed: bool,
    total: usize,
}

/// Project the conformance mapping into the deterministic published report.
fn project_report(mapping: &Mapping, implementation: String) -> PublishedReport {
    let mut requirements = BTreeMap::new();
    let (mut met, mut partial, mut deferred) = (0usize, 0usize, 0usize);
    for (id, r) in &mapping.requirements {
        match r.status.as_str() {
            "met" => met += 1,
            "partial" => partial += 1,
            "deferred" => deferred += 1,
            _ => {}
        }
        requirements.insert(
            id.clone(),
            PublishedRequirement {
                checks: r.checks.clone(),
                enforcement_level: r.enforcement_level.clone(),
                evidence: r.evidence.clone(),
                status: r.status.clone(),
                violation_evidence: r.violation_evidence.clone(),
                violation_tests: r.violation_tests.clone(),
            },
        );
    }
    PublishedReport {
        implementation,
        requirements,
        spec_version: mapping.spec_version.clone(),
        summary: PublishedSummary {
            deferred,
            met,
            partial,
            passed: partial == 0 && deferred == 0,
            total: mapping.requirements.len(),
        },
    }
}

/// Parse the stated spec version and requirement count from CONFORMANCE.md's
/// header line. Looks for "Spec v<version>" and "N requirements" patterns.
/// Returns (spec_version, requirement_count) or an error string.
fn parse_conformance_md_header(path: &str) -> Result<(String, usize), String> {
    let content = std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?;

    // Match the first line containing "Spec v" (e.g. "Ckeletin Spec v0.8.0")
    let spec_version = content
        .lines()
        .find_map(|line| {
            // Look for "Spec v<semver>" pattern
            let lower = line.to_lowercase();
            if lower.contains("spec v") {
                // Extract the version token following "v"
                let pos = line.to_lowercase().find("spec v")? + "spec v".len();
                let rest = &line[pos..];
                let ver: String = rest
                    .chars()
                    .take_while(|c| c.is_ascii_digit() || *c == '.')
                    .collect();
                if ver.is_empty() { None } else { Some(ver) }
            } else {
                None
            }
        })
        .ok_or_else(|| format!("{path}: could not find 'Spec v<version>' in header"))?;

    // Match "N requirements" (e.g. "40 requirements")
    let req_count = content
        .lines()
        .find_map(|line| {
            // Look for "<N> requirements" on a header line
            let parts: Vec<&str> = line.split_whitespace().collect();
            for (i, word) in parts.iter().enumerate() {
                if (*word == "requirements"
                    || *word == "requirements,"
                    || word.starts_with("requirements"))
                    && i > 0
                    && let Ok(n) = parts[i - 1]
                        .trim_matches(|c: char| !c.is_ascii_digit())
                        .parse::<usize>()
                {
                    return Some(n);
                }
            }
            None
        })
        .ok_or_else(|| format!("{path}: could not find 'N requirements' in header"))?;

    Ok((spec_version, req_count))
}

fn main() {
    let json_mode = std::env::args().any(|a| a == "--json");
    // `--refresh` fetches the latest spec from upstream and rewrites the
    // vendored requirements.json. Without it (the CI/gating default) the tool is
    // hermetic: it reads only the committed vendored spec, with no network and
    // no file writes.
    let refresh = std::env::args().any(|a| a == "--refresh");
    // `--report` (re)writes the published conformance-report.json. Without it,
    // the committed report is sync-checked against the mapping and must match.
    let write_report = std::env::args().any(|a| a == "--report");

    let mapping_content = match std::fs::read_to_string("conformance-mapping.toml") {
        Ok(c) => c,
        Err(e) => {
            let msg = format!("cannot read conformance-mapping.toml: {e}");
            if json_mode {
                println!(
                    "{}",
                    serde_json::to_string_pretty(
                        &serde_json::json!({ "status": "error", "error": msg })
                    )
                    .unwrap()
                );
            } else {
                eprintln!("Error: {msg}");
            }
            std::process::exit(1);
        }
    };

    let mapping: Mapping = match toml::from_str(&mapping_content) {
        Ok(m) => m,
        Err(e) => {
            let msg = format!("invalid mapping file: {e}");
            if json_mode {
                println!(
                    "{}",
                    serde_json::to_string_pretty(
                        &serde_json::json!({ "status": "error", "error": msg })
                    )
                    .unwrap()
                );
            } else {
                eprintln!("Error: {msg}");
            }
            std::process::exit(1);
        }
    };

    // ── Load requirement IDs from spec (replaces hardcoded list) ──
    let (expected_ids, spec_version) = load_spec_requirements(refresh, json_mode);

    // ── Spec version comparison (SSOT) ─────────────────────────
    // The mapping and the vendored requirements.json MUST target the same spec
    // version; a mismatch means the report is reasoning about the wrong
    // requirement set, so fail rather than warn (and don't silence it in JSON
    // mode). `just conform-refresh` updates the vendored spec for review.
    if mapping.spec_version != spec_version {
        let msg = format!(
            "conformance-mapping.toml targets spec {} but {} is spec {}; reconcile them",
            mapping.spec_version, VENDORED_REQUIREMENTS, spec_version
        );
        if json_mode {
            println!(
                "{}",
                serde_json::to_string_pretty(
                    &serde_json::json!({ "status": "error", "error": msg })
                )
                .unwrap()
            );
        } else {
            eprintln!("Error: {msg}.");
        }
        std::process::exit(1);
    }

    // ── ENF-005: Completeness check (both directions) ──────────
    // Forward: spec IDs not in mapping.
    let missing = find_unmapped(&expected_ids, &mapping.requirements);
    if !missing.is_empty() {
        let msg = format!(
            "unmapped requirements (CKSPEC-ENF-005 violation): {}",
            missing.join(", ")
        );
        if json_mode {
            println!(
                "{}",
                serde_json::to_string_pretty(
                    &serde_json::json!({ "status": "error", "error": msg })
                )
                .unwrap()
            );
        } else {
            eprintln!("FAILED — {msg}");
        }
        std::process::exit(1);
    }
    // Reverse: mapping IDs not in spec (extra/stale entries inflate totals).
    let extra = find_extra(&expected_ids, &mapping.requirements);
    if !extra.is_empty() {
        let msg = format!(
            "mapping contains entries not in the spec (stale/invented requirement IDs): {}",
            extra.join(", ")
        );
        if json_mode {
            println!(
                "{}",
                serde_json::to_string_pretty(
                    &serde_json::json!({ "status": "error", "error": msg })
                )
                .unwrap()
            );
        } else {
            eprintln!("FAILED — {msg}");
        }
        std::process::exit(1);
    }

    // ── ENF-008: Anchored conformance evidence ──────────────────
    // Every `met` requirement must carry at least one anchor (a check, a
    // violation test, or written violation_evidence). An unanchored met claim
    // fails the gate so it can't be published.
    let unanchored: Vec<&str> = mapping
        .requirements
        .iter()
        .filter(|(_, r)| lacks_anchor(r))
        .map(|(id, _)| id.as_str())
        .collect();
    if !unanchored.is_empty() {
        let msg = format!(
            "unanchored met requirements (CKSPEC-ENF-008): {}",
            unanchored.join(", ")
        );
        if json_mode {
            println!(
                "{}",
                serde_json::to_string_pretty(
                    &serde_json::json!({ "status": "error", "error": msg })
                )
                .unwrap()
            );
        } else {
            eprintln!("FAILED — {msg}");
            eprintln!(
                "  Each met requirement needs a check, a violation test, or violation_evidence."
            );
        }
        std::process::exit(1);
    }

    // ── Dangling anchor check ────────────────────────────────────
    // Every evidence anchor that names a file path must exist on disk. An anchor
    // of the form `file.rs::symbol` additionally requires that `symbol` appears
    // in the file. Dangling anchors are a hard failure so stale evidence cannot
    // be published.
    let dangling = find_dangling_anchors(&mapping.requirements);
    if !dangling.is_empty() {
        let lines: Vec<String> = dangling
            .iter()
            .map(|(id, msg)| format!("  {id}: {msg}"))
            .collect();
        let summary = format!(
            "dangling evidence anchors found ({} problem(s))",
            dangling.len()
        );
        if json_mode {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "status": "error",
                    "error": summary,
                    "details": dangling.iter().map(|(id, msg)| format!("{id}: {msg}")).collect::<Vec<_>>()
                }))
                .unwrap()
            );
        } else {
            eprintln!("FAILED — {summary}:");
            for line in &lines {
                eprintln!("{line}");
            }
        }
        std::process::exit(1);
    }

    // ── CONFORMANCE.md header sync check ────────────────────────
    // CONFORMANCE.md must declare the same spec version as the mapping, and its
    // stated requirement count must match the spec's count. This prevents the
    // prose from silently falling behind a spec bump (the incident that prompted
    // this gate).
    if !write_report {
        match parse_conformance_md_header(CONFORMANCE_MD) {
            Ok((md_version, md_count)) => {
                let mut header_errors: Vec<String> = Vec::new();
                if md_version != mapping.spec_version {
                    header_errors.push(format!(
                        "{CONFORMANCE_MD} states spec v{md_version} but mapping targets spec {}; update the prose header",
                        mapping.spec_version
                    ));
                }
                let expected_count = expected_ids.len();
                if md_count != expected_count {
                    header_errors.push(format!(
                        "{CONFORMANCE_MD} states {md_count} requirements but spec has {expected_count}; update the prose header"
                    ));
                }
                if !header_errors.is_empty() {
                    let msg = header_errors.join("; ");
                    if json_mode {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(
                                &serde_json::json!({ "status": "error", "error": msg })
                            )
                            .unwrap()
                        );
                    } else {
                        eprintln!("FAILED — {msg}");
                    }
                    std::process::exit(1);
                }
            }
            Err(e) => {
                let msg = format!("CONFORMANCE.md header parse failed: {e}");
                if json_mode {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(
                            &serde_json::json!({ "status": "error", "error": msg })
                        )
                        .unwrap()
                    );
                } else {
                    eprintln!("FAILED — {msg}");
                }
                std::process::exit(1);
            }
        }
    }

    // ── ENF-010: Published report (write, or sync-check vs mapping) ──
    let published = project_report(&mapping, detect_implementation_name());
    let generated =
        serde_json::to_string_pretty(&published).expect("serialize published report") + "\n";
    if write_report {
        if let Err(e) = std::fs::write(PUBLISHED_REPORT, &generated) {
            eprintln!("Error: cannot write {PUBLISHED_REPORT}: {e}");
            std::process::exit(1);
        }
        if !json_mode {
            eprintln!(
                "Wrote {PUBLISHED_REPORT} ({} requirements).",
                mapping.requirements.len()
            );
        }
        return;
    }
    let committed = std::fs::read_to_string(PUBLISHED_REPORT).unwrap_or_default();
    if committed != generated {
        let msg = format!(
            "{PUBLISHED_REPORT} is out of sync with conformance-mapping.toml (CKSPEC-ENF-010); run `just conform-report`"
        );
        if json_mode {
            println!(
                "{}",
                serde_json::to_string_pretty(
                    &serde_json::json!({ "status": "error", "error": msg })
                )
                .unwrap()
            );
        } else {
            eprintln!("FAILED — {msg}");
        }
        std::process::exit(1);
    }

    // ── Run checks and collect results ──────────────────────────
    let mut results = BTreeMap::new();
    let mut feedback = Vec::new();
    let mut met = 0usize;
    let mut partial = 0usize;
    let mut deferred = 0usize;
    let mut failed_checks = 0usize;

    for (req_id, req) in &mapping.requirements {
        let mut check_results = Vec::new();
        let mut vtest_results = Vec::new();

        // Run checks — capture output on failure so the user can diagnose it.
        for check_cmd in &req.checks {
            let (passed, failure_output) = run_check(check_cmd);
            if !passed {
                failed_checks += 1;
            }
            if !json_mode {
                let icon = if passed { "ok" } else { "FAIL" };
                println!("  {req_id:<20} {check_cmd} ... {icon}");
                if !passed
                    && let Some(out) = &failure_output
                    && !out.trim().is_empty()
                {
                    for line in out.lines() {
                        println!("    | {line}");
                    }
                }
            }
            check_results.push(CheckResult {
                command: check_cmd.clone(),
                passed,
            });
        }

        // Verify violation tests exist (ENF-006)
        for vt in &req.violation_tests {
            let exists = std::path::Path::new(vt).exists();
            if !exists {
                feedback.push(format!("{req_id}: violation test not found: {vt}"));
            }
            vtest_results.push(ViolationTestResult {
                path: vt.clone(),
                exists,
            });
        }

        // ENF-006: claims above honor-system need proof (violation_tests or violation_evidence)
        if lacks_enforcement_proof(req) {
            feedback.push(format!(
                "{req_id}: claims {} but has no violation test or evidence",
                req.enforcement_level
            ));
        }

        match req.status.as_str() {
            "met" => met += 1,
            "partial" => partial += 1,
            "deferred" => deferred += 1,
            _ => {}
        }

        results.insert(
            req_id.clone(),
            RequirementResult {
                title: req.title.clone(),
                status: req.status.clone(),
                enforcement_level: req.enforcement_level.clone(),
                evidence: req.evidence.clone(),
                checks: check_results,
                violation_tests: vtest_results,
            },
        );
    }

    let total = mapping.requirements.len();
    let today = current_date();

    let report = Report {
        implementation: detect_implementation_name(),
        spec_version: mapping.spec_version.clone(),
        report_date: today,
        summary: Summary {
            total,
            met,
            partial,
            deferred,
            failed_checks,
            feedback_signals: feedback.len(),
        },
        requirements: results,
        feedback,
    };

    // ── Output ──────────────────────────────────────────────────

    if json_mode {
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
    } else {
        println!();
        println!("── Results ──────────────────────────────────────────");
        println!();
        println!("  Requirements:  {} total", report.summary.total);
        println!("  Met:           {}", report.summary.met);
        if report.summary.partial > 0 {
            println!("  Partial:       {}", report.summary.partial);
        }
        if report.summary.deferred > 0 {
            println!("  Deferred:      {}", report.summary.deferred);
        }
        println!("  Failed checks: {}", report.summary.failed_checks);
        println!();

        if !report.feedback.is_empty() {
            println!("Feedback signals (ENF-007):");
            for f in &report.feedback {
                println!("  - {f}");
            }
            println!();
        }

        if report.summary.failed_checks > 0 {
            println!(
                "FAILED — {} check(s) did not pass.",
                report.summary.failed_checks
            );
            std::process::exit(1);
        }

        println!(
            "PASSED — {}/{} requirements met, {} deferred.",
            report.summary.met, report.summary.total, report.summary.deferred
        );
        if !report.feedback.is_empty() {
            println!(
                "         {} feedback signal(s) for spec review.",
                report.feedback.len()
            );
        }
    }

    if report.summary.failed_checks > 0 {
        std::process::exit(1);
    }
}

/// Run a shell check and return (passed, Option<combined output on failure>).
/// Output is captured so failures can surface their diagnostic text.
fn run_check(cmd: &str) -> (bool, Option<String>) {
    match Command::new("sh").arg("-c").arg(cmd).output() {
        Ok(output) => {
            if output.status.success() {
                (true, None)
            } else {
                let combined = format!(
                    "{}{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
                (false, Some(combined))
            }
        }
        Err(e) => (false, Some(format!("failed to spawn: {e}"))),
    }
}

/// Detect project name from the [[bin]] name in crates/cli/Cargo.toml.
fn detect_implementation_name() -> String {
    let content = match std::fs::read_to_string("crates/cli/Cargo.toml") {
        Ok(c) => c,
        Err(_) => return "unknown".to_string(),
    };
    let parsed: toml::Value = match toml::from_str(&content) {
        Ok(v) => v,
        Err(_) => return "unknown".to_string(),
    };
    // Read from [[bin]] array, first entry's name
    parsed
        .get("bin")
        .and_then(|b| b.as_array())
        .and_then(|arr| arr.first())
        .and_then(|entry| entry.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or("unknown")
        .to_string()
}

/// Get the current date in YYYY-MM-DD format without the chrono dependency.
/// Falls back to "unknown" rather than panicking if `date` is unavailable.
fn current_date() -> String {
    Command::new("date")
        .arg("+%Y-%m-%d")
        .output()
        .ok()
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if s.is_empty() { None } else { Some(s) }
        })
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mapping_with(ids: &[&str]) -> BTreeMap<String, RequirementMapping> {
        ids.iter()
            .map(|id| ((*id).to_string(), RequirementMapping::default()))
            .collect()
    }

    // ── ENF-005: completeness check catches an unmapped requirement ──

    #[test]
    fn find_unmapped_flags_a_requirement_missing_from_the_mapping() {
        let expected = vec!["CKSPEC-ARCH-001".to_string(), "CKSPEC-OUT-009".to_string()];
        let mapping = mapping_with(&["CKSPEC-ARCH-001"]);
        assert_eq!(
            find_unmapped(&expected, &mapping),
            vec!["CKSPEC-OUT-009".to_string()],
            "an id in the spec but not the mapping must be flagged"
        );
    }

    #[test]
    fn find_unmapped_is_empty_when_every_requirement_is_mapped() {
        let expected = vec!["CKSPEC-ARCH-001".to_string()];
        let mapping = mapping_with(&["CKSPEC-ARCH-001"]);
        assert!(find_unmapped(&expected, &mapping).is_empty());
    }

    // ── ENF-005 reverse: extra mapping entries (unknown IDs) fail ──

    #[test]
    fn find_extra_flags_a_mapping_entry_not_in_the_spec() {
        let expected = vec!["CKSPEC-ARCH-001".to_string()];
        let mapping = mapping_with(&["CKSPEC-ARCH-001", "CKSPEC-INVENTED-999"]);
        let extra = find_extra(&expected, &mapping);
        assert_eq!(extra, vec!["CKSPEC-INVENTED-999".to_string()]);
    }

    #[test]
    fn find_extra_is_empty_when_all_mapping_entries_are_in_spec() {
        let expected = vec!["CKSPEC-ARCH-001".to_string(), "CKSPEC-OUT-001".to_string()];
        let mapping = mapping_with(&["CKSPEC-ARCH-001", "CKSPEC-OUT-001"]);
        assert!(find_extra(&expected, &mapping).is_empty());
    }

    // ── ENF-006: proof requirement catches an unproven above-honor claim ──

    #[test]
    fn lacks_proof_flags_above_honor_claim_with_neither_test_nor_evidence() {
        let req = RequirementMapping {
            enforcement_level: "compile-time".to_string(),
            ..Default::default()
        };
        assert!(lacks_enforcement_proof(&req));
    }

    #[test]
    fn lacks_proof_is_satisfied_by_a_violation_test() {
        let req = RequirementMapping {
            enforcement_level: "compile-time".to_string(),
            violation_tests: vec!["some/violation.rs".to_string()],
            ..Default::default()
        };
        assert!(!lacks_enforcement_proof(&req));
    }

    #[test]
    fn lacks_proof_is_satisfied_by_violation_evidence() {
        let req = RequirementMapping {
            enforcement_level: "script".to_string(),
            violation_evidence: Some("the cli.rs JSON tests catch a regression".to_string()),
            ..Default::default()
        };
        assert!(!lacks_enforcement_proof(&req));
    }

    #[test]
    fn lacks_proof_exempts_honor_system_and_design_levels() {
        for level in ["honor-system", "design"] {
            let req = RequirementMapping {
                enforcement_level: level.to_string(),
                ..Default::default()
            };
            assert!(
                !lacks_enforcement_proof(&req),
                "{level} is exempt from the proof requirement"
            );
        }
    }

    // ── ENF-008: anchoring gate ─────────────────────────────────

    #[test]
    fn anchored_met_passes() {
        let by_evidence = RequirementMapping {
            status: "met".to_string(),
            violation_evidence: Some("analysis-with-evidence".to_string()),
            ..Default::default()
        };
        assert!(!lacks_anchor(&by_evidence));
        let by_check = RequirementMapping {
            status: "met".to_string(),
            checks: vec!["test -f X".to_string()],
            ..Default::default()
        };
        assert!(!lacks_anchor(&by_check));
    }

    #[test]
    fn unanchored_met_is_rejected() {
        let bare = RequirementMapping {
            status: "met".to_string(),
            ..Default::default()
        };
        assert!(lacks_anchor(&bare));
        // blank/whitespace violation_evidence is not an anchor
        let blank = RequirementMapping {
            status: "met".to_string(),
            violation_evidence: Some("  ".to_string()),
            ..Default::default()
        };
        assert!(lacks_anchor(&blank));
    }

    #[test]
    fn non_met_status_needs_no_anchor() {
        let deferred = RequirementMapping {
            status: "deferred".to_string(),
            ..Default::default()
        };
        assert!(!lacks_anchor(&deferred));
    }

    // ── Dangling anchor gate ────────────────────────────────────

    #[test]
    fn dangling_anchor_detects_nonexistent_file_path() {
        let mut mapping = BTreeMap::new();
        mapping.insert(
            "CKSPEC-TEST-999".to_string(),
            RequirementMapping {
                status: "met".to_string(),
                evidence: "crates/domain/tests/nonexistent_fixture.rs".to_string(),
                violation_evidence: Some(
                    "Structural: see crates/domain/tests/nonexistent_fixture.rs".to_string(),
                ),
                ..Default::default()
            },
        );
        let dangling = find_dangling_anchors(&mapping);
        // Both evidence and violation_evidence mention the nonexistent path
        assert!(
            !dangling.is_empty(),
            "a nonexistent file path in evidence must be reported as dangling"
        );
        assert!(
            dangling.iter().any(|(id, _)| id == "CKSPEC-TEST-999"),
            "the dangling anchor must be attributed to the correct requirement"
        );
    }

    #[test]
    fn dangling_anchor_passes_for_existing_file() {
        // Use a file we know exists in the repo
        let mut mapping = BTreeMap::new();
        mapping.insert(
            "CKSPEC-TEST-998".to_string(),
            RequirementMapping {
                status: "met".to_string(),
                evidence: "Cargo.toml".to_string(), // no slash, not picked as anchor
                checks: vec!["test -f Cargo.toml".to_string()],
                ..Default::default()
            },
        );
        let dangling = find_dangling_anchors(&mapping);
        assert!(
            dangling.is_empty(),
            "a non-path evidence string must not be flagged"
        );
    }

    #[test]
    fn dangling_anchor_violation_test_path_is_checked() {
        let mut mapping = BTreeMap::new();
        mapping.insert(
            "CKSPEC-TEST-997".to_string(),
            RequirementMapping {
                status: "met".to_string(),
                violation_tests: vec![
                    "crates/domain/tests/violations/dangling_does_not_exist.rs".to_string(),
                ],
                violation_evidence: Some("trybuild tests".to_string()),
                ..Default::default()
            },
        );
        let dangling = find_dangling_anchors(&mapping);
        assert!(
            dangling
                .iter()
                .any(|(_, msg)| msg.contains("dangling_does_not_exist.rs")),
            "a nonexistent violation_test path must be reported as dangling"
        );
    }

    // ── Dangling anchor: path.rs::symbol tokenization (item 4) ────

    #[test]
    fn tokenize_anchors_recognizes_rs_symbol_tokens() {
        // file.rs::symbol must be included so the symbol check is reachable.
        let tokens =
            tokenize_anchors("see crates/domain/src/logging.rs::validate_level for details");
        assert!(
            tokens
                .iter()
                .any(|t| t == "crates/domain/src/logging.rs::validate_level"),
            "path::symbol token must be collected; got: {tokens:?}"
        );
    }

    #[test]
    fn dangling_anchor_detects_missing_symbol_via_rs_symbol_token() {
        // Write a temp .rs file with a known extension (required for tokenizer),
        // then reference a symbol that does not appear in it.
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("fixture.rs");
        std::fs::write(&file_path, "fn real_symbol() {}\n").unwrap();
        let path_str = file_path.to_str().unwrap().to_string();
        let anchor = format!("{path_str}::nonexistent_symbol_xyz");
        let mut mapping = BTreeMap::new();
        mapping.insert(
            "CKSPEC-TEST-996".to_string(),
            RequirementMapping {
                status: "met".to_string(),
                evidence: anchor,
                checks: vec![format!("test -f {path_str}")],
                ..Default::default()
            },
        );
        let dangling = find_dangling_anchors(&mapping);
        assert!(
            !dangling.is_empty(),
            "a path.rs::symbol anchor where the symbol is absent must be flagged as dangling"
        );
        assert!(
            dangling
                .iter()
                .any(|(_, msg)| msg.contains("nonexistent_symbol_xyz")),
            "dangling message must name the missing symbol; got: {dangling:?}"
        );
    }

    #[test]
    fn dangling_anchor_passes_for_valid_rs_symbol_token() {
        // Write a temp .rs file with a known symbol; path::symbol anchor must
        // not be flagged when both the file and symbol exist.
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("fixture.rs");
        std::fs::write(&file_path, "fn existing_symbol() {}\n").unwrap();
        let path_str = file_path.to_str().unwrap().to_string();
        let anchor = format!("{path_str}::existing_symbol");
        let mut mapping = BTreeMap::new();
        mapping.insert(
            "CKSPEC-TEST-995".to_string(),
            RequirementMapping {
                status: "met".to_string(),
                evidence: anchor,
                checks: vec![format!("test -f {path_str}")],
                ..Default::default()
            },
        );
        let dangling = find_dangling_anchors(&mapping);
        assert!(
            dangling.is_empty(),
            "a valid path.rs::symbol anchor must not be flagged; got: {dangling:?}"
        );
    }

    // ── CONFORMANCE.md header parse ─────────────────────────────

    #[test]
    fn conformance_md_header_parse_extracts_version_and_count() {
        // Write a temp file with a known header
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_str().unwrap().to_string();
        std::fs::write(&path, "# Ckeletin Spec v0.8.0 — Rust Conformance Report\n\n**Total:** 40 requirements — 40 met\n").unwrap();
        let (ver, count) = parse_conformance_md_header(&path).unwrap();
        assert_eq!(ver, "0.8.0");
        assert_eq!(count, 40);
    }

    #[test]
    fn conformance_md_header_parse_rejects_missing_version() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_str().unwrap().to_string();
        std::fs::write(&path, "# Some report\n\n**Total:** 40 requirements\n").unwrap();
        assert!(parse_conformance_md_header(&path).is_err());
    }

    #[test]
    fn conformance_md_header_parse_rejects_missing_count() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_str().unwrap().to_string();
        std::fs::write(
            &path,
            "# Ckeletin Spec v0.8.0 — Rust Conformance\n\nNo count here.\n",
        )
        .unwrap();
        assert!(parse_conformance_md_header(&path).is_err());
    }

    // ── ENF-010: deterministic published report ─────────────────

    #[test]
    fn report_projection_is_deterministic() {
        let mut reqs = BTreeMap::new();
        reqs.insert(
            "CKSPEC-ZZZ-002".to_string(),
            RequirementMapping {
                status: "met".to_string(),
                enforcement_level: "script".to_string(),
                checks: vec!["c".to_string()],
                ..Default::default()
            },
        );
        reqs.insert(
            "CKSPEC-AAA-001".to_string(),
            RequirementMapping {
                status: "met".to_string(),
                violation_evidence: Some("e".to_string()),
                ..Default::default()
            },
        );
        let m = Mapping {
            spec_version: "9.9.9".to_string(),
            requirements: reqs,
        };
        let a = serde_json::to_string_pretty(&project_report(&m, "impl".to_string())).unwrap();
        let b = serde_json::to_string_pretty(&project_report(&m, "impl".to_string())).unwrap();
        assert_eq!(a, b, "projection must be deterministic");
        assert!(
            a.find("CKSPEC-AAA-001").unwrap() < a.find("CKSPEC-ZZZ-002").unwrap(),
            "requirement keys must be sorted"
        );
        assert!(
            a.find("\"checks\"").unwrap() < a.find("\"status\"").unwrap(),
            "per-requirement fields must be alphabetical"
        );
        assert!(
            !a.contains("report_date"),
            "the published report must carry no timestamp"
        );
    }

    #[test]
    fn sync_check_detects_drift() {
        let mut reqs = BTreeMap::new();
        reqs.insert(
            "X".to_string(),
            RequirementMapping {
                status: "met".to_string(),
                violation_evidence: Some("e".to_string()),
                ..Default::default()
            },
        );
        let m = Mapping {
            spec_version: "1.0.0".to_string(),
            requirements: reqs,
        };
        let generated =
            serde_json::to_string_pretty(&project_report(&m, "impl".to_string())).unwrap();
        let drifted = generated.replace("\"met\"", "\"partial\"");
        assert_ne!(
            generated, drifted,
            "a drifted committed report must differ from the regenerated one"
        );
    }
}
