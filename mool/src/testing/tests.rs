use super::target;

/// Verifies generated server names are distinct and accepted by backend identifiers.
#[test]
fn generated_names_are_unique_identifier_safe_values() {
    let first = target::generated_name();
    let second = target::generated_name();

    assert_ne!(first, second);
    assert!(first.starts_with("mool_test_"));
    assert!(first.chars().all(|character| character.is_ascii_lowercase()
        || character.is_ascii_digit()
        || character == '_'));
}

/// Verifies SQLite test configuration preserves pool settings and avoids the source target.
#[cfg(feature = "sqlite")]
#[test]
fn sqlite_targets_are_isolated_and_preserve_pool_settings() {
    let source = crate::DbConf::from_url("sqlite::memory:?cache=shared&max=3&min=0&lazy=true")
        .expect("valid SQLite source configuration");
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("isolated.sqlite");

    let target = target::sqlite_conf(&source, &path).expect("isolated SQLite target");

    assert_ne!(target.url, source.url);
    assert!(target.url.starts_with("sqlite:"));
    assert!(target.url.contains("cache=shared"));
    assert_eq!(target.max_connections, source.max_connections);
    assert_eq!(target.min_connections, source.min_connections);
    assert_eq!(target.lazy, source.lazy);
}

/// Verifies server target derivation retains transport options and pool configuration.
#[cfg(any(feature = "postgres", feature = "mysql", feature = "mariadb"))]
#[test]
fn server_targets_preserve_transport_and_replace_only_database_name() {
    let source = crate::DbConf::from_url(
        "postgres://user:password@localhost/source?sslmode=require&max=3&min=0&lazy=true",
    )
    .expect("valid server source configuration");

    let target = target::server_conf(&source, "mool_test_target").expect("isolated server target");

    assert!(target.url.contains("/mool_test_target"));
    assert!(target.url.contains("sslmode=require"));
    assert_eq!(target.max_connections, source.max_connections);
    assert_eq!(target.min_connections, source.min_connections);
    assert_eq!(target.lazy, source.lazy);
}
