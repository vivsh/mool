#![cfg(feature = "test-support")]

use std::time::Duration;

use mool as db;
use mool::DbSession;

#[cfg(feature = "migrations")]
static ROOT_MIGRATIONS: db::migrations::EmbeddedMigrations =
    db::migrations::embedded_migrations!("tests/fixtures/test_database_migrations");
#[cfg(feature = "migrations")]
static CHILD_MIGRATIONS: db::migrations::EmbeddedMigrations =
    db::migrations::embedded_migrations!("tests/fixtures/test_database_child_migrations");

/// Verifies setup creates an isolated lazy pool that executes SQL and tears down cleanly.
#[tokio::test]
#[ignore = "run through scripts/integration-tests.sh"]
async fn test_database_creates_executes_and_tears_down() {
    let mut database = db::testing::setup(test_conf())
        .create()
        .await
        .expect("create isolated test database");
    let target = database.conf().url.clone();

    assert_eq!(database.pool().as_sqlx().size(), 0);
    database
        .pool_mut()
        .execute(statement(
            "CREATE TABLE mool_test_items (id INTEGER PRIMARY KEY)",
        ))
        .await
        .expect("create isolated table");
    database.teardown().await.expect("tear down test database");
    assert!(!target_exists(&target).await);
}

/// Verifies an eager source configuration connects the returned pool during setup.
#[tokio::test]
#[ignore = "run through scripts/integration-tests.sh"]
async fn test_database_preserves_eager_pool_configuration() {
    let mut conf = test_conf();
    conf.lazy = false;
    conf.min_connections = 1;
    let database = db::testing::setup(conf)
        .create()
        .await
        .expect("create eager isolated test database");

    assert!(database.pool().as_sqlx().size() >= 1);
    database.teardown().await.expect("tear down test database");
}

/// Verifies dropping a test database schedules best-effort cleanup on the active Tokio runtime.
#[tokio::test]
#[ignore = "run through scripts/integration-tests.sh"]
async fn dropped_test_database_is_removed() {
    let database = db::testing::setup(test_conf())
        .create()
        .await
        .expect("create isolated test database");
    let target = database.conf().url.clone();
    drop(database);

    wait_for_target_removal(&target).await;
}

/// Verifies root and crate migration histories are applied before the pool is returned.
#[cfg(all(feature = "migrations", any(feature = "postgres", feature = "sqlite")))]
#[tokio::test]
#[ignore = "run through scripts/integration-tests.sh"]
async fn test_database_applies_registered_root_and_crate_migrations() {
    let mut registry = db::migrations::MigrationRegistry::new();
    registry
        .register(db::migrations::root_migration(&ROOT_MIGRATIONS))
        .expect("register root migrations");
    registry
        .register(db::migrations::crate_migration("child", &CHILD_MIGRATIONS))
        .expect("register crate migrations");

    let mut database = db::testing::setup(test_conf())
        .with_migrations(&registry)
        .create()
        .await
        .expect("create migrated test database");

    let root_marker: i64 = database
        .pool_mut()
        .fetch_scalar(statement("SELECT COUNT(*) FROM mool_test_marker"))
        .await
        .expect("query root migration table");
    let child_marker: i64 = database
        .pool_mut()
        .fetch_scalar(statement("SELECT COUNT(*) FROM mool_test_child_marker"))
        .await
        .expect("query crate migration table");

    assert_eq!(root_marker, 0);
    assert_eq!(child_marker, 0);
    database.teardown().await.expect("tear down test database");
}

/// Verifies an empty migration registry skips migration execution and keeps a lazy pool unopened.
#[cfg(all(feature = "migrations", any(feature = "postgres", feature = "sqlite")))]
#[tokio::test]
#[ignore = "run through scripts/integration-tests.sh"]
async fn test_database_skips_an_empty_migration_registry() {
    let registry = db::migrations::MigrationRegistry::new();
    let database = db::testing::setup(test_conf())
        .with_migrations(&registry)
        .create()
        .await
        .expect("create test database without migration execution");

    assert_eq!(database.pool().as_sqlx().size(), 0);
    database.teardown().await.expect("tear down test database");
}

/// Builds a lazy source configuration without reusing an application database target.
#[cfg(feature = "sqlite")]
fn test_conf() -> db::DbConf {
    db::DbConf {
        url: "sqlite::memory:".to_string(),
        min_connections: 0,
        max_connections: 1,
        lazy: true,
    }
}

/// Loads the server connection used only to provision isolated database targets.
#[cfg(any(feature = "postgres", feature = "mysql", feature = "mariadb"))]
fn test_conf() -> db::DbConf {
    let mut conf = db::DbConf::from_env().expect("test backend DATABASE_URL");
    conf.min_connections = 0;
    conf.lazy = true;
    conf
}

/// Renders static test SQL through Mool's normal statement boundary.
fn statement(sql: &str) -> db::Statement {
    db::query(sql)
        .to_statement()
        .expect("static test SQL has valid bindings")
}

/// Waits for the detached drop cleanup task to remove its isolated target.
async fn wait_for_target_removal(target: &str) {
    for _ in 0..100 {
        if !target_exists(target).await {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(
        !target_exists(target).await,
        "test database target was not removed"
    );
}

/// Checks whether a target remains after automatic cleanup is scheduled.
#[cfg(feature = "postgres")]
async fn target_exists(target: &str) -> bool {
    use db::sqlx::migrate::MigrateDatabase;

    <db::sqlx::Postgres as MigrateDatabase>::database_exists(target)
        .await
        .expect("inspect PostgreSQL test target")
}

/// Checks whether a target remains after automatic cleanup is scheduled.
#[cfg(feature = "sqlite")]
async fn target_exists(target: &str) -> bool {
    use db::sqlx::migrate::MigrateDatabase;

    <db::sqlx::Sqlite as MigrateDatabase>::database_exists(target)
        .await
        .expect("inspect SQLite test target")
}

/// Checks whether a target remains after automatic cleanup is scheduled.
#[cfg(any(feature = "mysql", feature = "mariadb"))]
async fn target_exists(target: &str) -> bool {
    use db::sqlx::migrate::MigrateDatabase;

    <db::sqlx::MySql as MigrateDatabase>::database_exists(target)
        .await
        .expect("inspect MySQL-family test target")
}
