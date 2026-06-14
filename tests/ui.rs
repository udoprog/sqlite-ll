//! Compile-fail tests for the `Statements` derive macro.
//!
//! These assert the diagnostics emitted for misuse of the `#[sql(...)]`
//! attributes, including that a `read_only` collection cannot embed a nested
//! collection that is not itself read-only.

#[cfg_attr(miri, ignore)]
#[test]
fn ui() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/*.rs");
}
