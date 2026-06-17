/// Architecture violation tests for the infrastructure crate (CKSPEC-ENF-006).
///
/// Each test verifies that infrastructure cannot reach "up" to domain or cli.
/// These are meta-tests: they test the enforcement mechanism.

#[test]
fn infrastructure_cannot_import_domain() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/violations/infra_imports_domain.rs");
}

#[test]
fn infrastructure_cannot_import_clap() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/violations/infra_imports_clap.rs");
}
