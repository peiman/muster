/// Architecture violation tests for the domain crate (CKSPEC-ENF-006).
///
/// Each test verifies that a specific architectural boundary is enforced
/// at compile time. If any violation file compiles successfully, the
/// corresponding CKSPEC requirement's enforcement claim is invalid.
///
/// These are meta-tests: they test the enforcement mechanism, not the
/// requirement itself.

#[test]
fn domain_cannot_import_clap() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/violations/domain_imports_clap.rs");
}

#[test]
fn domain_cannot_import_tracing() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/violations/domain_imports_tracing.rs");
}

#[test]
fn domain_cannot_import_figment() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/violations/domain_imports_figment.rs");
}

#[test]
fn domain_cannot_import_infrastructure() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/violations/domain_imports_infrastructure.rs");
}

#[test]
fn domain_cannot_import_ckeletin() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/violations/domain_imports_ckeletin.rs");
}
