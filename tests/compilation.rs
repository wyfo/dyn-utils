#[cfg(all(not(miri), feature = "macros"))]
#[test]
fn compilation() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compilation/errors.rs");
    // check expansion of impls to keep them up to date with macro modifications
    macrotest::expand("tests/compilation/impls.rs");
}
