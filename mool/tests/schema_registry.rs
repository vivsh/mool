#![cfg(feature = "migrations")]

mod common;

use common::{Post, User};
use mool as db;

fn root_schema() -> db::Schema {
    db::schema(db::Dialect::Postgres).model::<User>().build()
}

fn crate_schema() -> db::Schema {
    db::schema(db::Dialect::Postgres).model::<Post>().build()
}

/// Verifies root and crate schema sources build the expected namespace-specific schemas.
#[test]
fn migration_registry_builds_schema_for_namespaces() {
    let mut registry = db::MigrationRegistry::new();
    registry
        .register_schema(db::root_schema(root_schema))
        .unwrap();
    registry
        .register_schema(db::crate_schema("blog", crate_schema))
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
    let mut left = db::MigrationRegistry::new();
    left.register_schema(db::root_schema(root_schema)).unwrap();
    let mut right = db::MigrationRegistry::new();
    right
        .register_schema(db::crate_schema("blog", crate_schema))
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
    let mut registry = db::MigrationRegistry::new();
    let err = registry
        .register_schema(db::crate_schema("bad/name", crate_schema))
        .unwrap_err();

    assert_eq!(
        err.to_string(),
        "invalid migration namespace 'bad/name': namespace cannot contain '/'"
    );
}
