//! License integrity test — asserts the SHA-256 of LICENSE-APACHE matches the
//! canonical Apache License 2.0 text from https://www.apache.org/licenses/LICENSE-2.0.txt
//!
//! This test exists to prevent silent drift: if the license file is accidentally
//! truncated, reformatted, or replaced with a paraphrase, this test turns red
//! immediately. The hash below was computed from the verbatim canonical text
//! fetched from Apache.org on 2026-06-09.
//!
//! If you legitimately need to update the license (e.g. Apache publishes a new
//! version), update the hash here to match the new canonical file.

use sha2::{Digest, Sha256};
use std::fs;

/// SHA-256 of the verbatim Apache License 2.0 text from
/// https://www.apache.org/licenses/LICENSE-2.0.txt (fetched 2026-06-09).
const CANONICAL_SHA256: &str = "cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30";

#[test]
fn license_apache_matches_canonical_hash() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR must be set when running cargo test");
    // CARGO_MANIFEST_DIR is crates/cli/, so ../../LICENSE-APACHE is the repo root
    let license_path = std::path::Path::new(&manifest_dir).join("../../LICENSE-APACHE");
    let contents = fs::read(&license_path)
        .unwrap_or_else(|e| panic!("Cannot read {}: {e}", license_path.display()));

    let mut hasher = Sha256::new();
    hasher.update(&contents);
    // Hex-encode byte-by-byte: digest 0.11 dropped LowerHex on the output
    // array, and this form is stable across digest versions.
    let actual_hash: String = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();

    assert_eq!(
        actual_hash, CANONICAL_SHA256,
        "LICENSE-APACHE has drifted from the canonical Apache License 2.0 text.\n\
         Expected SHA-256: {CANONICAL_SHA256}\n\
         Actual SHA-256:   {actual_hash}\n\
         Replace LICENSE-APACHE with the verbatim text from \
         https://www.apache.org/licenses/LICENSE-2.0.txt"
    );
}
