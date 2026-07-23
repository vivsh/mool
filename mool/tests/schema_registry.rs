#![cfg(feature = "migrations")]

mod common;

use common::User;
use mool as db;
use mool::gaman::core::MigrationStore;

#[derive(Debug, Clone, db::Model)]
#[table(name = "posts")]
struct BlogPost {
    id: i64,
    title: String,
}

static ROOT_MIGRATIONS: db::migrations::EmbeddedMigrations =
    db::migrations::embedded_migrations!("tests/fixtures/migrations");
static CRATE_MIGRATIONS: db::migrations::EmbeddedMigrations =
    db::migrations::embedded_migrations!("tests/fixtures/migrations");

fn root_schema() -> db::schema::Schema {
    db::schema()
        .model::<User>()
        .build()
        .expect("valid root test schema")
}

fn crate_schema() -> db::schema::Schema {
    db::schema()
        .model::<BlogPost>()
        .build()
        .expect("valid crate test schema")
}

/// Verifies root and crate schema sources build the expected namespace-specific schemas.
#[test]
fn migration_registry_builds_schema_for_namespaces() {
    let mut registry = db::migrations::MigrationRegistry::new();
    registry
        .register_schema(db::migrations::root_schema(root_schema))
        .unwrap();
    registry
        .register_schema(db::migrations::crate_schema("blog", crate_schema))
        .unwrap();

    assert!(
        registry
            .schema_for(None)
            .unwrap()
            .tables
            .contains_key("users")
    );
    assert!(
        registry
            .schema_for(Some("blog"))
            .unwrap()
            .tables
            .contains_key("posts")
    );
}

/// Verifies registry merge keeps root and crate schema sources from both registries.
#[test]
fn migration_registry_merges_schema_sources() {
    let mut left = db::migrations::MigrationRegistry::new();
    left.register_schema(db::migrations::root_schema(root_schema))
        .unwrap();
    let mut right = db::migrations::MigrationRegistry::new();
    right
        .register_schema(db::migrations::crate_schema("blog", crate_schema))
        .unwrap();

    left.merge(right).unwrap();

    assert!(left.schema_for(None).unwrap().tables.contains_key("users"));
    assert!(
        left.schema_for(Some("blog"))
            .unwrap()
            .tables
            .contains_key("posts")
    );
}

/// Verifies invalid namespaces are rejected before they reach migration routing.
#[test]
fn migration_registry_rejects_invalid_schema_namespace() {
    let mut registry = db::migrations::MigrationRegistry::new();
    let err = registry
        .register_schema(db::migrations::crate_schema("bad/name", crate_schema))
        .unwrap_err();

    assert_eq!(
        err.to_string(),
        "invalid migration namespace 'bad/name': namespace cannot contain '/'"
    );
}

/// Verifies the registry is a Gaman migration store with qualified crate identities.
#[tokio::test]
async fn migration_registry_implements_gaman_store() {
    let mut registry = db::migrations::MigrationRegistry::new();
    registry
        .register(db::migrations::root_migration(&ROOT_MIGRATIONS))
        .expect("root migration source");
    registry
        .register(db::migrations::crate_migration("blog", &CRATE_MIGRATIONS))
        .expect("crate migration source");

    let migrations = registry.load_all().await.expect("registered migrations");
    let ids = migrations
        .iter()
        .map(|migration| migration.id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        ids,
        vec![
            "0001_first",
            "0002_second",
            "blog/0001_first",
            "blog/0002_second"
        ]
    );
    assert!(
        migrations
            .iter()
            .all(|migration| migration.dependencies.is_empty())
    );
}
