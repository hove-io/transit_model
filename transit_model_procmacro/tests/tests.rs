#[test]
fn compile_error() {
    let t = trybuild::TestCases::new();
    t.pass("tests/01-get-corresponding.rs");
    t.compile_fail("tests/02-invalid-weight.rs");
    t.compile_fail("tests/03-non-supported-argument.rs");
}
