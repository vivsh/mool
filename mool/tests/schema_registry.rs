#![cfg(feature = "migrations")]

pub mod common;

use std::{path::Path, sync::Arc};

use common::User;
use db::migrations::engine::{
    Config, MakeCommand, MigrationCommand, MigrationStore, NativeRunnerFactory,
};
use mool as db;

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

fn root_schema() -> Result<db::schema::Schema, db::schema::SchemaLoadError> {
    db::schema().model::<User>().build()
}

fn crate_schema() -> Result<db::schema::Schema, db::schema::SchemaLoadError> {
    db::schema().model::<BlogPost>().build()
}

fn failing_schema() -> Result<db::schema::Schema, db::schema::SchemaLoadError> {
    Err(db::schema::SchemaLoadError::Validation(
        db::schema::SchemaValidationError::Invalid("task schema is unavailable".to_string()),
    ))
}

/// Verifies root schema generation includes application and crate contributions.
#[test]
fn migration_registry_builds_combined_root_schema() {
    let mut registry = db::migrations::MigrationRegistry::new();
    registry
        .register_schema(db::migrations::root_schema(root_schema))
        .unwrap();
    registry
        .register_schema(db::migrations::crate_schema("blog", crate_schema))
        .unwrap();

    let root = registry.schema_for(None).unwrap();
    assert!(root.tables.contains_key("users"));
    assert!(root.tables.contains_key("posts"));

    let blog = registry.schema_for(Some("blog")).unwrap();
    assert!(!blog.tables.contains_key("users"));
    assert!(blog.tables.contains_key("posts"));
}

/// Verifies registry merge retains deterministic root and crate schema contributions.
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

    let root = left.schema_for(None).unwrap();
    assert!(root.tables.contains_key("users"));
    assert!(root.tables.contains_key("posts"));
}

/// Verifies fallible schema builders preserve the namespace and source error.
#[test]
fn migration_registry_preserves_schema_source_errors() {
    let mut registry = db::migrations::MigrationRegistry::new();
    registry
        .register_schema(db::migrations::crate_schema("tasks", failing_schema))
        .unwrap();

    let error = registry.schema_for(Some("tasks")).unwrap_err();
    assert!(matches!(
        error,
        db::migrations::MigrationError::SchemaSource { ref namespace, .. }
            if namespace == "tasks"
    ));
    assert!(error.to_string().contains("task schema is unavailable"));
}

/// Verifies prevalidated schema values can be registered without a callback.
#[test]
fn migration_registry_registers_prevalidated_schema_values() {
    let mut registry = db::migrations::MigrationRegistry::new();
    let schema = root_schema().unwrap();
    registry
        .register_schema(db::migrations::root_schema_value(schema).unwrap())
        .unwrap();

    assert!(
        registry
            .schema_for(None)
            .unwrap()
            .tables
            .contains_key("users")
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

/// Verifies registry loads root and crate histories with qualified identities.
#[tokio::test]
async fn migration_registry_implements_combined_store() {
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
}

/// Verifies root saves are durable and visible to the next registry load.
#[tokio::test]
async fn migration_registry_saves_and_reloads_root_migrations() {
    let directory = tempfile::tempdir().unwrap();
    let root = dynamic_root(directory.path(), &[]);
    let mut registry = db::migrations::MigrationRegistry::new();
    registry
        .register(db::migrations::root_migration(root))
        .unwrap();

    let migration = empty_migration("0001_tasks");
    registry.save(&migration).await.unwrap();

    let stored = directory.path().join("0001_tasks.yaml");
    assert!(stored.is_file());
    assert!(
        std::fs::read_to_string(stored)
            .unwrap()
            .contains("operations: []")
    );
    assert_eq!(
        registry
            .load_all()
            .await
            .unwrap()
            .iter()
            .map(|migration| migration.id.as_str())
            .collect::<Vec<_>>(),
        vec!["0001_tasks"]
    );
}

/// Verifies the native runner can generate through a registry-backed root store.
#[tokio::test]
async fn native_runner_factory_generates_into_registry_root() {
    let directory = tempfile::tempdir().unwrap();
    let root = dynamic_root(directory.path(), &[]);
    let mut registry = db::migrations::MigrationRegistry::new();
    registry
        .register(db::migrations::root_migration(root))
        .unwrap();

    let config = Config::new(
        "sqlite::memory:".to_string(),
        directory.path().to_path_buf(),
        directory.path().join("schema.yaml"),
        db::migrations::Dialect::Sqlite,
    );
    let mut runner = NativeRunnerFactory::from_store(config, Arc::new(registry)).build();

    let result = runner
        .run_command(&MigrationCommand::Make(MakeCommand::Empty {
            name: "initial_tasks".to_string(),
        }))
        .await
        .unwrap();

    assert!(matches!(
        result,
        db::migrations::engine::CommandResult::Make(_)
    ));
    assert!(
        std::fs::read_dir(directory.path())
            .unwrap()
            .flatten()
            .any(|entry| entry
                .path()
                .extension()
                .is_some_and(|value| value == "yaml"))
    );
}

/// Verifies generated crate ids cannot escape the root migration directory.
#[tokio::test]
async fn migration_registry_rejects_namespaced_root_saves() {
    let directory = tempfile::tempdir().unwrap();
    let root = dynamic_root(directory.path(), &[]);
    let mut registry = db::migrations::MigrationRegistry::new();
    registry
        .register(db::migrations::root_migration(root))
        .unwrap();

    let error = registry
        .save(&empty_migration("tasks/0001_init"))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("cannot contain namespaces"));
    assert!(
        std::fs::read_dir(directory.path())
            .unwrap()
            .next()
            .is_none()
    );
}

/// Verifies disk content cannot silently diverge from an embedded root migration.
#[tokio::test]
async fn migration_registry_rejects_embedded_disk_content_conflicts() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("0001_init.yaml"), "operations: []\n").unwrap();
    let root = dynamic_root(
        directory.path(),
        &[("0001_init", "atomic: false\noperations: []\n")],
    );
    let mut registry = db::migrations::MigrationRegistry::new();
    registry
        .register(db::migrations::root_migration(root))
        .unwrap();

    let error = registry.load_all().await.unwrap_err();
    assert!(error.to_string().contains("content differs"));
}

fn dynamic_root(
    directory: &Path,
    files: &'static [(&'static str, &'static str)],
) -> &'static db::migrations::EmbeddedMigrations {
    let directory = Box::leak(directory.to_string_lossy().into_owned().into_boxed_str());
    Box::leak(Box::new(db::migrations::EmbeddedMigrations {
        files,
        dir: directory,
        children: &[],
    }))
}

fn empty_migration(id: &str) -> db::gaman::Migration {
    db::gaman::Migration {
        id: id.to_string(),
        dependencies: Vec::new(),
        operations: Vec::new(),
        atomic: true,
    }
}
