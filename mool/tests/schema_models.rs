mod common;

use common::{Account, AuditLog, Membership, Post, PostWithAuthor, User, col};
use mool as db;

#[derive(Debug, Clone, db::Model)]
#[table(name = "catalog_items")]
struct CatalogItem {
    #[column(primary_key)]
    id: i64,
    #[column(index_name = "catalog_items_slug_idx")]
    slug: String,
    #[column(unique_name = "catalog_items_sku_key")]
    sku: String,
    #[column(check = "price_cents >= 0")]
    price_cents: i64,
}

#[derive(Debug, Clone, db::Model)]
#[table(name = "invalid_references")]
struct InvalidReference {
    #[column(primary_key)]
    id: i64,
    #[column(reference = "missing.id")]
    missing_id: i64,
}

/// Verifies generated table metadata used by schema and query generation.
#[test]
fn model_derive_generates_table_metadata() {
    let table = <Account as db::IntoTable>::into_table(&db::Dialect::Postgres);
    let audit_table = <AuditLog as db::IntoTable>::into_table(&db::Dialect::Postgres);

    assert_eq!(table.name, "accounts");
    assert_eq!(table.schema.as_deref(), Some("auth"));
    assert!(col(&table, "id").primary_key);
    assert_eq!(col(&table, "email_address").col_type, "citext");
    assert!(col(&table, "nickname").nullable);
    assert_eq!(audit_table.name, "audit_log");
    assert_eq!(<Account as db::Model>::primary_key_column(), Some("id"));
}

/// Verifies composite primary key metadata keeps the explicit name and column order.
#[test]
fn model_derive_preserves_composite_primary_key_metadata() {
    let table = <Membership as db::IntoTable>::into_table(&db::Dialect::Postgres);
    let primary_key = table.primary_key.as_ref().expect("primary key");

    assert_eq!(primary_key.name, "memberships_identity");
    assert_eq!(primary_key.columns, vec!["tenant_id", "user_id"]);
    assert_eq!(
        <Membership as db::Model>::primary_key_columns(),
        &["tenant_id", "user_id"]
    );
}

/// Verifies record flattening and references become selectable columns and join metadata.
#[test]
fn record_derive_exposes_flattened_reference_metadata() {
    assert_eq!(<PostWithAuthor as db::Record>::record_table_name(), "posts");
    assert_eq!(
        <PostWithAuthor as db::Record>::record_root_name(),
        Some("post")
    );
    assert_eq!(
        <PostWithAuthor as db::Record>::record_column_names(),
        vec![
            "id",
            "author_id",
            "title",
            "published",
            "created_at",
            "subtitle",
            "author.id",
            "author.email",
            "author.active",
        ]
    );
    let references = <PostWithAuthor as db::Record>::record_references();
    assert_eq!(references.len(), 1);
    assert_eq!(references[0].logical_name, "author");
    assert_eq!(references[0].columns[0].from, "author_id");
}

/// Verifies model schema building includes multiple model tables and inferred column types.
#[test]
fn schema_builder_collects_model_tables() {
    let schema = db::schema(db::Dialect::Postgres)
        .model::<User>()
        .model::<Post>()
        .build()
        .expect("valid model schema");

    let users = common::table(&schema, "users");
    let posts = common::table(&schema, "posts");

    assert_eq!(col(users, "email").col_type, "text");
    assert_eq!(col(posts, "author_id").col_type, "bigint");
    assert_eq!(
        col(posts, "created_at").col_type,
        "timestamp with time zone"
    );
    assert!(col(posts, "subtitle").nullable);
    assert_eq!(posts.foreign_keys.len(), 1);
    assert_eq!(posts.foreign_keys[0].name, "posts_author_id_fkey");
    assert_eq!(posts.foreign_keys[0].columns, vec!["author_id"]);
    assert_eq!(posts.foreign_keys[0].to_table, "users");
    assert_eq!(posts.foreign_keys[0].to_columns, vec!["id"]);
}

/// Verifies enum-aware schema building exposes dialect validation failures.
#[test]
fn schema_builder_returns_validation_errors() {
    let error = db::schema(db::Dialect::Postgres)
        .model::<InvalidReference>()
        .build()
        .expect_err("invalid foreign key must fail schema validation");

    assert!(
        error
            .to_string()
            .contains("referenced table missing not found")
    );
}

/// Verifies field-level index, unique, and check metadata are preserved.
#[test]
fn model_derive_preserves_indexes_uniques_and_checks() {
    let table = <CatalogItem as db::IntoTable>::into_table(&db::Dialect::Postgres);

    assert_eq!(table.indexes.len(), 1);
    assert_eq!(table.indexes[0].name, "catalog_items_slug_idx");
    assert_eq!(table.indexes[0].columns, vec!["slug"]);
    assert!(!table.indexes[0].unique);
    assert!(table.constraints.iter().any(|constraint| matches!(
        constraint,
        db::Constraint::Unique { name, columns }
            if name == "catalog_items_sku_key" && columns == &vec!["sku".to_string()]
    )));
    assert_eq!(
        col(&table, "price_cents").check.as_deref(),
        Some("price_cents >= 0")
    );
}
