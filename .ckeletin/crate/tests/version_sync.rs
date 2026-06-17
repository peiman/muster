//! Version sync test — ensures the crate's Cargo.toml version matches the
//! canonical VERSION file one level up. Any future drift is caught immediately
//! at `just check`, forcing the merge captain to keep them in sync (Principle 9).

use std::fs;

#[test]
fn crate_version_matches_version_file() {
    // CARGO_MANIFEST_DIR is .ckeletin/crate/, so ../VERSION is .ckeletin/VERSION
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR must be set when running cargo test");
    let version_file = std::path::Path::new(&manifest_dir).join("../VERSION");
    let expected = fs::read_to_string(&version_file)
        .unwrap_or_else(|e| panic!("Cannot read {}: {e}", version_file.display()))
        .trim()
        .to_string();
    let actual = env!("CARGO_PKG_VERSION");
    assert_eq!(
        actual, expected,
        "Crate CARGO_PKG_VERSION ({actual}) does not match .ckeletin/VERSION ({expected}). \
         Update .ckeletin/crate/Cargo.toml [package].version to match."
    );
}
