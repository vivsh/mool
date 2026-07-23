/// Verifies public macro contracts with compile-pass and compile-fail fixtures.
#[test]
fn public_macro_contracts_compile_as_documented() {
    let tests = trybuild::TestCases::new();
    #[cfg(feature = "postgres")]
    tests.pass("tests/compile/pass/public_api.rs");
    tests.pass("tests/compile/pass/sqlx_compat.rs");
    tests.pass("tests/compile/pass/mock_available_debug.rs");
    tests.pass("tests/compile/pass/typed_contracts.rs");
    #[cfg(feature = "migrations")]
    tests.pass("tests/compile/pass/embedded_migrations.rs");
    #[cfg(feature = "migrations")]
    tests.pass("tests/compile/pass/migration_engine.rs");
    #[cfg(all(feature = "migrations", feature = "sqlite"))]
    tests.pass("tests/compile/pass/migration_engine_sqlite.rs");
    #[cfg(all(feature = "migrations", feature = "postgres"))]
    tests.pass("tests/compile/pass/migration_engine_postgres.rs");

    tests.compile_fail("tests/compile/fail/filterable_invalid_op.rs");
    tests.compile_fail("tests/compile/fail/model_sql_enum_conflicting_type.rs");
    tests.compile_fail("tests/compile/fail/sql_enum_data_variant.rs");
    tests.compile_fail("tests/compile/fail/sql_enum_int_missing_code.rs");
    tests.compile_fail("tests/compile/fail/column_type_mismatch.rs");
    tests.compile_fail("tests/compile/fail/projection_type_mismatch.rs");
    tests.compile_fail("tests/compile/fail/variable_type_mismatch.rs");
    tests.compile_fail("tests/compile/fail/write_type_mismatch.rs");
    #[cfg(any(feature = "mysql", feature = "mariadb"))]
    tests.compile_fail("tests/compile/fail/unsupported_returning.rs");
    #[cfg(not(feature = "postgres"))]
    tests.compile_fail("tests/compile/fail/unsupported_postgres_array.rs");
    #[cfg(feature = "sqlite")]
    tests.compile_fail("tests/compile/fail/unsupported_row_lock.rs");
    #[cfg(feature = "postgres")]
    tests.compile_fail("tests/compile/fail/unnest_nested_array.rs");
}
