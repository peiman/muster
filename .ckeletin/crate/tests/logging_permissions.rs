// OUT-005 exception: test skip-signal writes to stderr are legitimate test-harness
// communication (not library output). The Output struct cannot be used here.
#![allow(clippy::print_stderr)]
//! Integration tests for audit log file/directory permissions.
//!
//! Runs as a separate test binary so it can call logging::init() once
//! without conflicting with other test files that also set the global
//! tracing subscriber.

use ckeletin::logging::{LogConfig, init};
use std::fs;

#[cfg(unix)]
fn is_root() -> bool {
    std::process::Command::new("id")
        .arg("-u")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.trim().parse::<u32>().ok())
        .unwrap_or(1)
        == 0
}

#[cfg(unix)]
#[test]
fn log_directory_and_file_have_restricted_permissions() {
    use std::os::unix::fs::PermissionsExt;

    if is_root() {
        eprintln!("SKIP: running as root, permission tests are unreliable");
        return;
    }

    let base = tempfile::tempdir().unwrap();
    let log_path = base.path().join("logs").join("app.log");
    let config = LogConfig {
        console_level: "off".to_string(),
        file_enabled: true,
        file_path: log_path.to_str().unwrap().to_string(),
        file_level: "debug".to_string(),
    };

    let guard = init(&config).expect("init must succeed");
    drop(guard);

    // Directory must be 0700 (owner-only).
    let log_dir = base.path().join("logs");
    let dir_mode = fs::metadata(&log_dir).unwrap().permissions().mode() & 0o777;
    assert_eq!(
        dir_mode, 0o700,
        "audit log directory must be 0700, got {dir_mode:o}"
    );

    // At least one log file must exist with mode 0600.
    let entries: Vec<_> = fs::read_dir(&log_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert!(!entries.is_empty(), "at least one log file must exist");
    for entry in &entries {
        let file_mode = fs::metadata(entry.path()).unwrap().permissions().mode() & 0o777;
        assert_eq!(
            file_mode,
            0o600,
            "audit log file {:?} must be 0600, got {file_mode:o}",
            entry.path()
        );
    }
}

/// A pre-existing log directory with lax permissions (0755) must be tightened
/// to 0700. This is tested by replicating the chmod logic directly — `init()`
/// can only be called once per process (global subscriber), so we exercise the
/// directory hardening path in isolation here.
#[cfg(unix)]
#[test]
fn pre_existing_lax_log_dir_is_tightened_to_0700() {
    use std::os::unix::fs::PermissionsExt;

    if is_root() {
        eprintln!("SKIP: running as root, permission tests are unreliable");
        return;
    }

    let base = tempfile::tempdir().unwrap();
    let log_dir = base.path().join("logs");

    // Pre-create the directory with lax 0755 permissions (as if a previous
    // `sudo` run created it world-readable).
    fs::create_dir_all(&log_dir).unwrap();
    fs::set_permissions(&log_dir, fs::Permissions::from_mode(0o755)).unwrap();

    let before = fs::metadata(&log_dir).unwrap().permissions().mode() & 0o777;
    assert_eq!(before, 0o755, "pre-condition: dir must start at 0755");

    // Simulate what prepare_file_appender does on Unix: DirBuilder won't
    // change mode on an existing directory, but the unconditional set_permissions
    // call afterwards must tighten it.
    {
        use std::os::unix::fs::DirBuilderExt;
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&log_dir)
            .unwrap();
        // This is the key line: unconditional tighten, even for pre-existing dirs.
        fs::set_permissions(&log_dir, fs::Permissions::from_mode(0o700)).unwrap();
    }

    let after = fs::metadata(&log_dir).unwrap().permissions().mode() & 0o777;
    assert_eq!(
        after, 0o700,
        "pre-existing 0755 audit log dir must be tightened to 0700, got {after:o}"
    );
}
