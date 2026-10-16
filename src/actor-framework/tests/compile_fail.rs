#![cfg(feature = "tokio")]

#[test]
fn invalid_actor_routes_do_not_compile() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/*.rs");
}
