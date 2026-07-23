use mool as db;

/// Verifies Mool consumes pool options while retaining SQLx transport options.
#[test]
fn database_url_preserves_transport_options() {
    let config =
        db::DbConf::from_url("postgres://localhost/mool?sslmode=require&max=20&min=2&lazy=true")
            .expect("valid pool URL");

    assert_eq!(config.max_connections, 20);
    assert_eq!(config.min_connections, 2);
    assert!(config.lazy);
    assert!(config.url.contains("sslmode=require"));
    assert!(!config.url.contains("max=20"));
}

/// Verifies malformed pool options return configuration errors instead of defaults.
#[test]
fn database_url_rejects_invalid_pool_options() {
    let invalid_max = db::DbConf::from_url("postgres://localhost/mool?max=many")
        .expect_err("invalid max must fail");
    let invalid_range = db::DbConf::from_url("postgres://localhost/mool?min=5&max=2")
        .expect_err("invalid pool range must fail");

    assert_eq!(invalid_max.code(), "configuration_error");
    assert_eq!(invalid_range.code(), "configuration_error");
}

/// Verifies the selected backend supplies a matching default URL scheme.
#[test]
fn selected_backend_default_url_matches_transport() {
    let config = db::DbConf::default();

    #[cfg(feature = "sqlite")]
    assert!(config.url.starts_with("sqlite:"));
    #[cfg(feature = "postgres")]
    assert!(config.url.starts_with("postgres:"));
    #[cfg(any(feature = "mysql", feature = "mariadb"))]
    assert!(config.url.starts_with("mysql:"));
}
