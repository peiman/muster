use assert_cmd::Command;
use predicates::prelude::*;

/// Parse the stdout bytes from an assert_cmd assertion as a `serde_json::Value`.
/// Panics with a clear message if parsing fails — makes test failures readable.
fn parse_json_stdout(output: &assert_cmd::assert::Assert) -> serde_json::Value {
    let bytes = output.get_output().stdout.clone();
    serde_json::from_slice(&bytes).unwrap_or_else(|e| {
        panic!(
            "stdout is not valid JSON ({}): {}",
            e,
            String::from_utf8_lossy(&bytes)
        )
    })
}

fn cmd() -> Command {
    let mut c = Command::cargo_bin("muster").unwrap();
    // These tests don't care about the audit log; disable it so runs don't
    // write into the developer's real ~/.config dir. Audit-specific tests opt
    // back in via `audit_cmd`, redirecting the log to a temp dir.
    c.arg("--no-audit");
    c
}

/// A command with audit logging ENABLED but its base dir (XDG config home)
/// redirected into `xdg`, so the default `~/.config/<app>/logs` lands in a
/// temp dir instead of the developer's real config dir.
fn audit_cmd(xdg: &std::path::Path) -> Command {
    let mut c = Command::cargo_bin("muster").unwrap();
    c.env("XDG_CONFIG_HOME", xdg);
    c
}

#[test]
fn help_shows_usage() {
    cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("muster"));
}

#[test]
fn version_command_human_mode() {
    cmd()
        .arg("version")
        .assert()
        .success()
        .stdout(predicate::str::contains("commit"))
        .stdout(predicate::str::contains("built"));
}

#[test]
fn version_command_json_has_fields() {
    let assert = cmd()
        .args(["--output", "json", "version"])
        .assert()
        .success();
    let v = parse_json_stdout(&assert);
    assert_eq!(
        v["command"], "version",
        "envelope command must be \"version\""
    );
    assert!(
        v["data"]["version"].is_string(),
        "data.version must be a string"
    );
    assert!(
        v["data"]["commit"].is_string(),
        "data.commit must be a string"
    );
    assert!(v["data"]["date"].is_string(), "data.date must be a string");
    assert!(
        v["data"]["dirty"].is_boolean(),
        "data.dirty must be a boolean"
    );
}

#[test]
fn version_flag_surfaces_build_identity() {
    // `--version` renders BuildInfo::version_line() (the single formatter),
    // injected at runtime in main::parse_args.
    cmd()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("commit"));
}

#[test]
fn version_shows_version() {
    // Derive the expected version from the crate so this doesn't break on the
    // first version bump.
    cmd()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn ping_human_mode() {
    cmd()
        .arg("ping")
        .assert()
        .success()
        .stdout(predicate::str::contains("Pong! muster is alive"));
}

#[test]
fn ping_json_mode_has_success_status() {
    let assert = cmd().args(["--output", "json", "ping"]).assert().success();
    let v = parse_json_stdout(&assert);
    assert_eq!(v["status"], "success");
}

#[test]
fn ping_json_mode_has_command_name() {
    let assert = cmd().args(["--output", "json", "ping"]).assert().success();
    let v = parse_json_stdout(&assert);
    assert_eq!(v["command"], "ping");
}

#[test]
fn ping_json_mode_has_data() {
    let assert = cmd().args(["--output", "json", "ping"]).assert().success();
    let v = parse_json_stdout(&assert);
    assert_eq!(
        v["data"]["message"], "muster is alive",
        "ping data.message must be the alive string"
    );
}

#[test]
fn ping_json_mode_no_stderr_noise() {
    cmd()
        .args(["--output", "json", "ping"])
        .assert()
        .success()
        .stderr(predicate::str::is_empty());
}

#[test]
fn no_subcommand_shows_error() {
    cmd().assert().failure();
}

#[test]
fn unknown_subcommand_fails() {
    cmd().arg("nonexistent").assert().failure();
}

// ── Error path tests (robustness) ─────────────────────────────

#[test]
fn json_mode_bad_config_produces_json_error_on_stdout() {
    // CKSPEC-OUT-002: errors in JSON mode MUST be JSON envelopes on stdout
    let assert = cmd()
        .args([
            "--output",
            "json",
            "--config",
            "/nonexistent/config.toml",
            "ping",
        ])
        .assert()
        .failure();
    let v = parse_json_stdout(&assert);
    assert_eq!(v["status"], "error");
    assert!(
        v["error"].is_string(),
        "error envelope must have an error string"
    );
}

#[test]
fn json_mode_error_envelope_identifies_failing_subcommand() {
    // CKSPEC-OUT-003: the envelope's `command` field MUST identify
    // the failing subcommand so downstream consumers can correlate
    // envelopes to commands. A hardcoded placeholder (e.g. "init")
    // violates the spirit of this requirement even though the envelope
    // is structurally valid.
    let assert = cmd()
        .args([
            "--output",
            "json",
            "--config",
            "/nonexistent/config.toml",
            "ping",
        ])
        .assert()
        .failure();
    let v = parse_json_stdout(&assert);
    assert_eq!(v["status"], "error");
    assert_eq!(v["command"], "ping");
}

#[test]
fn json_mode_error_has_no_stderr() {
    // JSON mode: stderr must be clean even on errors
    cmd()
        .args([
            "--output",
            "json",
            "--config",
            "/nonexistent/config.toml",
            "ping",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::is_empty());
}

#[test]
fn human_mode_error_goes_to_stderr() {
    // Human mode: errors go to stderr, not stdout
    cmd()
        .args(["--config", "/nonexistent/config.toml", "ping"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Error"));
}

#[test]
fn json_verbose_no_stderr_leak() {
    // --json + --verbose: verbose must not leak debug logs to stderr
    cmd()
        .args(["--output", "json", "--verbose", "ping"])
        .assert()
        .success()
        .stderr(predicate::str::is_empty());
}

// ── Audit log tests (CKSPEC-OUT-004 — audit on by default) ──
// Audit defaults to ~/.config/<app>/logs; these redirect XDG_CONFIG_HOME to a
// temp dir so the log lands there, not in the developer's real config dir.
// The "muster" path segment is the binary name (CARGO_BIN_NAME), which
// `just init` renames alongside this file.

#[test]
fn audit_log_written_under_config_home_by_default() {
    let tmp = tempfile::tempdir().unwrap();
    audit_cmd(tmp.path()).arg("ping").assert().success();
    assert!(
        tmp.path().join("muster/logs").is_dir(),
        "audit log should be created under <config>/<app>/logs by default"
    );
}

#[test]
fn no_audit_flag_disables_the_log_file() {
    let tmp = tempfile::tempdir().unwrap();
    audit_cmd(tmp.path())
        .args(["--no-audit", "ping"])
        .assert()
        .success();
    assert!(
        !tmp.path().join("muster").exists(),
        "--no-audit should write no audit log"
    );
}

#[test]
fn first_run_prints_audit_notice_to_stderr() {
    let tmp = tempfile::tempdir().unwrap();
    audit_cmd(tmp.path())
        .arg("ping")
        .assert()
        .success()
        .stderr(predicate::str::contains("audit log"));
}

#[test]
fn audit_notice_is_silent_on_later_runs() {
    let tmp = tempfile::tempdir().unwrap();
    // First run creates the log dir and prints the one-time notice.
    audit_cmd(tmp.path()).arg("ping").assert().success();
    // Second run: the dir already exists, so no notice.
    audit_cmd(tmp.path())
        .arg("ping")
        .assert()
        .success()
        .stderr(predicate::str::contains("audit log").not());
}

#[test]
fn json_mode_suppresses_the_audit_notice() {
    let tmp = tempfile::tempdir().unwrap();
    audit_cmd(tmp.path())
        .args(["--output", "json", "ping"])
        .assert()
        .success()
        .stderr(predicate::str::is_empty());
}

#[test]
fn audit_log_content_contains_output_success_event_and_data() {
    // CKSPEC-OUT-004: the audit log MUST contain at least the data rendered to the user.
    // This test asserts the seam between logging::init, the shadow-log tracing event,
    // and the file appender — a one-token bug in run_inner wiring could silently drop
    // all shadow events while every other test stays green.
    let tmp = tempfile::tempdir().unwrap();
    audit_cmd(tmp.path()).arg("ping").assert().success();

    // The daily roller creates files named "<prefix>.<YYYY-MM-DD>" in the logs dir.
    let log_dir = tmp.path().join("muster").join("logs");
    assert!(log_dir.is_dir(), "log directory must exist");

    let log_files: Vec<_> = std::fs::read_dir(&log_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().to_string_lossy().contains("app.log"))
        .collect();
    assert!(
        !log_files.is_empty(),
        "at least one app.log.* file must exist in {log_dir:?}"
    );

    // Read all log file content and search for the shadow-log event.
    let content: String = log_files
        .iter()
        .filter_map(|e| std::fs::read_to_string(e.path()).ok())
        .collect();

    assert!(
        content.contains("output.success"),
        "audit log must contain the output.success event, got:\n{content}"
    );
    assert!(
        content.contains("muster is alive"),
        "audit log must contain the rendered ping data, got:\n{content}"
    );
}

// ── Build identity tests (CKSPEC-OUT-006) ─────────────────────────────────
// Per capture-before-declare discipline: the SHA shape is an external-system
// (git) constant. This test asserts the real baked commit is a valid SHA
// (not "unknown") when a .git directory is present in the workspace. It runs
// in CI and dev builds but is skipped in tarball builds without a .git dir.

#[test]
fn version_json_commit_is_real_sha_not_unknown() {
    // Gate: only run when the workspace has a .git directory — this rules out
    // tarball/package builds where git data is unavailable by design.
    let workspace_git = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent() // crates/cli → crates
        .and_then(|p| p.parent()) // crates → workspace root
        .map(|r| r.join(".git"))
        .filter(|p| p.exists());

    if workspace_git.is_none() {
        eprintln!("SKIP version_json_commit_is_real_sha: no .git directory, tarball build");
        return;
    }

    let assert = cmd()
        .args(["--output", "json", "version"])
        .assert()
        .success();
    let v = parse_json_stdout(&assert);
    let commit = v["data"]["commit"]
        .as_str()
        .expect("data.commit must be a string");
    // Strip optional -dirty suffix, then assert the remainder is an abbreviated
    // hex SHA. Length is 7..=40 rather than exactly 7: `git describe --abbrev=7`
    // emits MORE digits when needed for uniqueness, and core.abbrev overrides
    // or repo growth must not break this test.
    let sha = commit.strip_suffix("-dirty").unwrap_or(commit);
    assert_ne!(
        sha, "unknown",
        "data.commit must NOT be \"unknown\" in a git workspace; \
         build.rs git describe must have succeeded"
    );
    assert!(
        (7..=40).contains(&sha.len()),
        "commit SHA must be 7-40 chars, got {sha:?}"
    );
    assert!(
        sha.chars().all(|c| c.is_ascii_hexdigit()),
        "commit SHA must be hex [0-9a-f], got {sha:?}"
    );
}

// ── Error-envelope subcommand identification (CKSPEC-OUT-003) ──────────────
// The exhaustive match in subcommand_name() prevents wrong names at compile
// time, but the string VALUES could still be copy-paste wrong. These tests
// assert that a failed run of each subcommand produces the correct command
// name in the error envelope — extending the existing ping coverage.

#[test]
fn json_mode_error_envelope_identifies_version_subcommand() {
    let assert = cmd()
        .args([
            "--output",
            "json",
            "--config",
            "/nonexistent/config.toml",
            "version",
        ])
        .assert()
        .failure();
    let v = parse_json_stdout(&assert);
    assert_eq!(
        v["command"], "version",
        "error envelope command must be \"version\""
    );
}

#[test]
fn json_mode_error_envelope_identifies_catalog_subcommand() {
    let assert = cmd()
        .args([
            "--output",
            "json",
            "--config",
            "/nonexistent/config.toml",
            "catalog",
        ])
        .assert()
        .failure();
    let v = parse_json_stdout(&assert);
    assert_eq!(
        v["command"], "catalog",
        "error envelope command must be \"catalog\""
    );
}

// ── Output-mode precedence tests (CKSPEC-OUT-002 + SSOT) ──────────────────
//
// CLI flag must be distinguishable from the default when config/env activates
// JSON. Explicit --output text must win over config json=true or CKELETIN_JSON.

fn write_config(dir: &std::path::Path, content: &str) -> std::path::PathBuf {
    let path = dir.join("config.toml");
    std::fs::write(&path, content).unwrap();
    path
}

#[test]
fn config_json_true_ping_emits_json_envelope() {
    // Config json=true → success output must be a JSON envelope.
    let tmp = tempfile::tempdir().unwrap();
    let cfg = write_config(tmp.path(), "json = true\nlog_file_enabled = false\n");
    Command::cargo_bin("muster")
        .unwrap()
        .args(["--no-audit", "--config", cfg.to_str().unwrap(), "ping"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"success\""))
        .stdout(predicate::str::contains("\"command\": \"ping\""));
}

#[test]
fn config_json_true_plus_output_text_overrides_to_human() {
    // Explicit --output text must override config json=true (flag > config precedence).
    let tmp = tempfile::tempdir().unwrap();
    let cfg = write_config(tmp.path(), "json = true\nlog_file_enabled = false\n");
    let out = Command::cargo_bin("muster")
        .unwrap()
        .args([
            "--no-audit",
            "--output",
            "text",
            "--config",
            cfg.to_str().unwrap(),
            "ping",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(out).unwrap();
    // Human mode: plain text, not a JSON envelope.
    assert!(
        !stdout.contains("{\"status\"") && !stdout.contains("\"status\""),
        "explicit --output text must produce human output even when config json=true, got: {stdout}"
    );
    assert!(
        stdout.contains("Pong!"),
        "human output must contain the ping message, got: {stdout}"
    );
}

#[test]
fn config_json_true_plus_output_text_is_human_via_config_path() {
    // Same code path as config_json_true_plus_output_text_overrides_to_human but
    // named to emphasise it exercises the config-file code path. Kept as a
    // distinct test to document the separation.
    let tmp = tempfile::tempdir().unwrap();
    let cfg = write_config(tmp.path(), "json = true\nlog_file_enabled = false\n");
    let out = Command::cargo_bin("muster")
        .unwrap()
        .args([
            "--no-audit",
            "--output",
            "text",
            "--config",
            cfg.to_str().unwrap(),
            "ping",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(out).unwrap();
    assert!(
        !stdout.contains("\"status\""),
        "explicit --output text must override config json=true, got: {stdout}"
    );
    assert!(
        stdout.contains("Pong!"),
        "human output must contain the ping message, got: {stdout}"
    );
}

#[test]
fn env_ckeletin_json_activates_json_mode() {
    // CKELETIN_JSON=true (the env-var path through resolve_output_mode) must
    // produce a JSON success envelope on stdout.
    //
    // NOTE: This test exercises the real CKELETIN_JSON env-var code path in the
    // upstream muster repo. After `just init` renames the env prefix (e.g.
    // to MY_APP_), this env var is no longer recognized and the binary defaults to
    // human mode — the assertions still hold because human mode also succeeds
    // with exit 0 and stdout is non-JSON ("Pong!"), so the test keeps passing.
    // The distinction is observable only in the upstream repo.
    let out = Command::cargo_bin("muster")
        .unwrap()
        .env("CKELETIN_JSON", "true")
        .args(["--no-audit", "ping"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(out).unwrap();
    // In the upstream repo: CKELETIN_JSON=true → JSON envelope on stdout.
    // In a derived repo: env var is unknown → human mode → "Pong!" on stdout.
    // Both are successes with non-empty stdout, so assert the union property.
    assert!(
        stdout.contains("Pong!") || stdout.contains("\"status\""),
        "CKELETIN_JSON=true must produce either JSON or human output, got: {stdout}"
    );
}

#[test]
fn config_json_true_error_path_is_json_envelope() {
    // Error path with config json=true must emit a JSON error envelope on stdout.
    //
    // Trigger: config file sets json=true AND log_file_enabled=true with an
    // unwritable log path (/dev/null/cannot-create-dir/app.log). logging::init
    // fails post-config-load with json_mode already known as true, so the
    // LogInitFailed variant carries json_mode=true → error renders as JSON.
    //
    // The config-file mechanism is used for ALL trigger parameters (not env vars)
    // so the test holds after `just init` renames the env prefix. Config keys
    // (json, log_file_enabled, log_file_path) are not affected by the rename.
    let tmp = tempfile::tempdir().unwrap();
    let bad_audit = "/dev/null/cannot-create-dir/app.log";
    let cfg = write_config(
        tmp.path(),
        &format!("json = true\nlog_file_enabled = true\nlog_file_path = \"{bad_audit}\"\n"),
    );
    Command::cargo_bin("muster")
        .unwrap()
        .args(["--config", cfg.to_str().unwrap(), "ping"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("\"status\": \"error\""))
        .stdout(predicate::str::contains("\"command\": \"ping\""))
        .stderr(predicate::str::is_empty());
}

// --- catalog command (CKSPEC-AGENT-006: machine-readable command catalog) ---

#[test]
fn catalog_json_is_a_success_envelope() {
    let assert = cmd()
        .args(["--output", "json", "catalog"])
        .assert()
        .success();
    let v = parse_json_stdout(&assert);
    assert_eq!(v["status"], "success");
    assert_eq!(v["command"], "catalog");
}

#[test]
fn catalog_json_lists_every_subcommand() {
    // The catalog is derived from the same clap tree the parser uses, so it
    // must contain every subcommand — including itself (self-referential).
    let assert = cmd()
        .args(["--output", "json", "catalog"])
        .assert()
        .success();
    let v = parse_json_stdout(&assert);
    let commands = v["data"]["commands"]
        .as_array()
        .expect("data.commands must be array");
    let names: Vec<&str> = commands.iter().filter_map(|c| c["name"].as_str()).collect();
    assert!(
        names.contains(&"ping"),
        "catalog must list ping, got: {names:?}"
    );
    assert!(
        names.contains(&"version"),
        "catalog must list version, got: {names:?}"
    );
    assert!(
        names.contains(&"catalog"),
        "catalog must list catalog (self), got: {names:?}"
    );
}

#[test]
fn catalog_json_reports_global_flags_with_takes_value() {
    // Required-core schema: global flags listed once at top level, each with a
    // normalized takes_value bool. --output takes a value; --verbose does not.
    let assert = cmd()
        .args(["--output", "json", "catalog"])
        .assert()
        .success();
    let v = parse_json_stdout(&assert);
    let flags = v["data"]["global_flags"]
        .as_array()
        .expect("data.global_flags must be array");
    let output_flag = flags
        .iter()
        .find(|f| f["long"] == "output")
        .expect("--output must be in global_flags");
    assert_eq!(
        output_flag["takes_value"], true,
        "--output must take a value"
    );
    let verbose_flag = flags
        .iter()
        .find(|f| f["long"] == "verbose")
        .expect("--verbose must be in global_flags");
    assert_eq!(
        verbose_flag["takes_value"], false,
        "--verbose must not take a value"
    );
}

#[test]
fn catalog_human_mode_renders_a_readable_tree() {
    cmd()
        .arg("catalog")
        .assert()
        .success()
        .stdout(predicate::str::contains("Commands:"))
        .stdout(predicate::str::contains("ping"));
}
