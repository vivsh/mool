/// Verifies public macro contracts with compile-pass and compile-fail fixtures.
#[cfg(feature = "compile-contracts")]
#[test]
fn public_macro_contracts_compile_as_documented() {
    let tests = trybuild::TestCases::new();
    #[cfg(feature = "postgres")]
    tests.pass("tests/compile/pass/public_api.rs");
    tests.pass("tests/compile/pass/sqlx_compat.rs");
    tests.pass("tests/compile/pass/datetime_contracts.rs");
    tests.pass("tests/compile/pass/mock_available_debug.rs");
    tests.pass("tests/compile/pass/typed_contracts.rs");
    #[cfg(feature = "migrations")]
    tests.pass("tests/compile/pass/embed_migrations.rs");
    #[cfg(feature = "migrations")]
    tests.pass("tests/compile/pass/embedded_migrations_legacy.rs");
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
    tests.compile_fail("tests/compile/fail/legacy_field_attribute.rs");
    tests.compile_fail("tests/compile/fail/legacy_schema_attribute.rs");
    tests.compile_fail("tests/compile/fail/legacy_validate_attribute.rs");
    #[cfg(feature = "migrations")]
    tests.compile_fail("tests/compile/fail/embed_migrations_missing.rs");
    #[cfg(feature = "migrations")]
    tests.compile_fail("tests/compile/fail/embed_migrations_directory_entry.rs");
    tests.compile_fail("tests/compile/fail/model_duplicate_column.rs");
    tests.compile_fail("tests/compile/fail/model_conflicting_column_flags.rs");
    tests.compile_fail("tests/compile/fail/model_malformed_reference.rs");
    #[cfg(not(feature = "time"))]
    tests.compile_fail("tests/compile/fail/datetime_naive_portable.rs");
    #[cfg(not(feature = "time"))]
    tests.compile_fail("tests/compile/fail/datetime_zoned_portable.rs");
    #[cfg(feature = "time")]
    tests.compile_fail("tests/compile/fail/datetime_time_naive_portable.rs");
    #[cfg(any(feature = "mysql", feature = "mariadb"))]
    tests.compile_fail("tests/compile/fail/unsupported_returning.rs");
    #[cfg(not(feature = "postgres"))]
    tests.compile_fail("tests/compile/fail/unsupported_postgres_array.rs");
    #[cfg(feature = "sqlite")]
    tests.compile_fail("tests/compile/fail/model_native_array_sqlite.rs");
    #[cfg(any(feature = "mysql", feature = "mariadb"))]
    tests.compile_fail("tests/compile/fail/model_native_array_mysql.rs");
    #[cfg(feature = "postgres")]
    tests.compile_fail("tests/compile/fail/model_array_missing_pg_metadata.rs");
    #[cfg(feature = "sqlite")]
    tests.compile_fail("tests/compile/fail/unsupported_row_lock.rs");
    #[cfg(all(feature = "postgres", target_os = "linux"))]
    tests.compile_fail("tests/compile/fail/unnest_nested_array.rs");
}
