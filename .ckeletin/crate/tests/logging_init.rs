// OUT-005 exception: test skip-signal writes to stderr are legitimate test-harness
// communication (not library output). The Output struct cannot be used here.
#![allow(clippy::print_stderr)]
//! Integration tests for logging::init().
//!
//! Each test runs in a separate process (cargo test runs each test file
//! as a separate binary), so we can call init() once per file without
//! conflicting with other tests that set the global subscriber.
//!
//! This file tests the file-logging path — the code at 59% coverage.
//! Permission tests live in logging_permissions.rs (separate binary).

use ckeletin::logging::{LogConfig, init};
use std::fs;

#[cfg(unix)]
#[test]
fn prepare_file_appender_errors_not_panics_on_unwritable_dir() {
    // root bypasses file permissions; skip to avoid false pass.
    // Check via `id -u` which doesn't require libc.
    let uid = std::process::Command::new("id")
        .arg("-u")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.trim().parse::<u32>().ok())
        .unwrap_or(1);
    if uid == 0 {
        eprintln!("SKIP: running as root, permission tests are unreliable");
        return;
    }
    let dir = tempfile::tempdir().unwrap();

    // Make the PARENT read-only so the log dir itself cannot be created.
    // (A read-only dir owned by the test user no longer suffices: init
    // self-heals it via the unconditional 0700 chmod — owners may always
    // chmod their own dirs. Creating a subdir under a 0555 parent fails
    // with EPERM before the chmod line is reached, which also matches the
    // real attack — a root-owned dir from a sudo run — for a non-owner.)
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o555)).unwrap();

    let log_path = dir.path().join("subdir").join("app.log");
    let config = LogConfig {
        console_level: "off".to_string(),
        file_enabled: true,
        file_path: log_path.to_str().unwrap().to_string(),
        file_level: "debug".to_string(),
    };

    // Must return Err, NOT panic (exit 101).
    let result = init(&config);
    assert!(
        result.is_err(),
        "init() must return Err on permission-denied directory, not panic"
    );

    // Restore permissions so tempdir cleanup works.
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn init_with_file_logging_creates_log_file_and_returns_guard() {
    let dir = tempfile::tempdir().unwrap();
    let log_path = dir.path().join("test.log");

    let config = LogConfig {
        console_level: "off".to_string(), // suppress stderr in test
        file_enabled: true,
        file_path: log_path.to_str().unwrap().to_string(),
        file_level: "debug".to_string(),
    };

    let guard = init(&config);
    assert!(guard.is_ok(), "init() should succeed with valid file path");

    // Emit a tracing event — it should land in the file
    tracing::info!(test = "logging_init", "test event");

    // Drop the guard to flush the non-blocking writer
    drop(guard);

    // The log directory should exist (created by init)
    assert!(dir.path().exists());

    // Check that SOME log file was created in the directory
    // (tracing-appender adds date suffixes, so the exact name varies)
    let entries: Vec<_> = fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .contains("test.log")
        })
        .collect();
    assert!(
        !entries.is_empty(),
        "Should have created at least one log file"
    );
}
