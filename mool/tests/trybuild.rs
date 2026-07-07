/// Verifies public macro contracts with compile-pass and compile-fail fixtures.
#[test]
fn public_macro_contracts_compile_as_documented() {
    let tests = trybuild::TestCases::new();
    tests.pass("tests/compile/pass/public_api.rs");
    tests.pass("tests/compile/pass/sqlx_compat.rs");
    tests.pass("tests/compile/pass/mock_available_debug.rs");
    #[cfg(feature = "migrations")]
    tests.pass("tests/compile/pass/embedded_migrations.rs");

    tests.compile_fail("tests/compile/fail/filterable_invalid_op.rs");
    tests.compile_fail("tests/compile/fail/model_sql_enum_conflicting_type.rs");
    tests.compile_fail("tests/compile/fail/sql_enum_data_variant.rs");
    tests.compile_fail("tests/compile/fail/sql_enum_int_missing_code.rs");
}
