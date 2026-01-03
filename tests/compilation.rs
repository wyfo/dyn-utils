#[cfg(not(miri))]
#[test]
fn compilation() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compilation/errors.rs");
}
