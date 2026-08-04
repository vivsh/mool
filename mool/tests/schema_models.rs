pub mod common;

use common::{Account, AuditLog, Membership, PostWithAuthor, col};
#[cfg(any(feature = "postgres", feature = "sqlite"))]
use common::{Post, User};
use mool as db;

type CreatedAt = chrono::DateTime<chrono::Utc>;
type MaybeCreatedAt = Option<CreatedAt>;
type Identifier = sqlx::types::Uuid;
type Metadata = sqlx::types::Json<serde_json::Value>;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct AppMetadata {
    source: String,
}

#[derive(Debug, Clone, db::Model)]
#[table(name = "inferred_type_rows")]
struct InferredTypeRow {
    #[column(primary_key)]
    id: Identifier,
    optional_id: Option<Identifier>,
    created_at: CreatedAt,
    processed_at: MaybeCreatedAt,
    metadata: serde_json::Value,
    typed_metadata: Metadata,
    app_metadata: sqlx::types::Json<AppMetadata>,
    optional_metadata: Option<Metadata>,
    bytes: Vec<u8>,
    #[column(type = "timestamp with time zone")]
    explicit_timestamp: CreatedAt,
    #[column(type = "json")]
    ordered_document: serde_json::Value,
}

#[cfg(feature = "time")]
type EventTime = time::OffsetDateTime;

#[cfg(feature = "time")]
#[derive(Debug, Clone, db::Model)]
#[table(name = "inferred_time_rows")]
struct InferredTimeRow {
    #[column(primary_key)]
    id: i64,
    occurred_at: EventTime,
    optional_at: Option<EventTime>,
    local_at: time::PrimitiveDateTime,
    event_date: time::Date,
}

#[cfg(feature = "postgres")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, db::SqlEnum)]
#[sql_enum(
    name = "array_status",
    storage = "native_postgres",
    rename_all = "snake_case"
)]
enum ArrayStatus {
    Ready,
    Complete,
}

#[cfg(feature = "postgres")]
#[derive(Debug, Clone, sqlx::Type)]
#[sqlx(transparent)]
struct CustomKey(i64);

#[cfg(feature = "postgres")]
#[derive(Debug, Clone, db::Model)]
#[table(name = "inferred_array_rows")]
struct InferredArrayRow {
    #[column(primary_key)]
    id: i64,
    identifiers: Vec<Identifier>,
    timestamps: Vec<CreatedAt>,
    documents: Vec<serde_json::Value>,
    typed_documents: Vec<Metadata>,
    custom_keys: Vec<CustomKey>,
    statuses: Vec<ArrayStatus>,
    #[cfg(feature = "time")]
    time_timestamps: Vec<EventTime>,
}

fn active_dialect() -> db::gaman::core::Dialect {
    #[cfg(feature = "postgres")]
    return db::gaman::core::Dialect::Postgres;
    #[cfg(feature = "sqlite")]
    return db::gaman::core::Dialect::Sqlite;
    #[cfg(feature = "mysql")]
    return db::gaman::core::Dialect::Mysql;
    #[cfg(feature = "mariadb")]
    return db::gaman::core::Dialect::Mariadb;
}

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
#[table(name = "extension_rows")]
struct ExtensionRow {
    #[column(primary_key)]
    id: i64,
    label: String,
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
    let dialect = active_dialect();
    let table = <Account as db::schema::IntoTable>::into_table(&dialect);
    let audit_table = <AuditLog as db::schema::IntoTable>::into_table(&dialect);

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
    let table = <Membership as db::schema::IntoTable>::into_table(&active_dialect());
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
#[cfg(any(feature = "postgres", feature = "sqlite"))]
#[test]
fn schema_builder_collects_model_tables() {
    let builder: db::SchemaBuilder = db::schema();
    let schema = builder
        .model::<User>()
        .model::<Post>()
        .build()
        .expect("valid model schema");

    let users = common::table(&schema, "users");
    let posts = common::table(&schema, "posts");

    assert_eq!(col(users, "email").col_type, "text");
    #[cfg(feature = "postgres")]
    assert_eq!(col(posts, "author_id").col_type, "bigint");
    #[cfg(feature = "sqlite")]
    assert_eq!(col(posts, "author_id").col_type, "integer");
    #[cfg(feature = "postgres")]
    assert_eq!(
        col(posts, "created_at").col_type,
        "timestamp with time zone"
    );
    #[cfg(feature = "sqlite")]
    assert_eq!(col(posts, "created_at").col_type, "timestamptz");
    assert!(col(posts, "subtitle").nullable);
    assert_eq!(posts.foreign_keys.len(), 1);
    assert_eq!(posts.foreign_keys[0].name, "posts_author_id_fkey");
    assert_eq!(posts.foreign_keys[0].columns, vec!["author_id"]);
    assert_eq!(posts.foreign_keys[0].to_table, "users");
    assert_eq!(posts.foreign_keys[0].to_columns, vec!["id"]);
}

/// Verifies generic table extensions preserve opaque constraints on every backend.
#[test]
fn schema_builder_extends_modeled_tables() {
    let schema = db::schema()
        .model::<ExtensionRow>()
        .extend_table("extension_rows", |table| {
            table.opaque_constraint(
                "extension_rows_label_nonempty",
                "CONSTRAINT extension_rows_label_nonempty CHECK (length(label) > 0)",
            )
        })
        .build()
        .expect("extended schema");

    assert!(
        schema.tables["extension_rows"]
            .constraints
            .iter()
            .any(|value| value.is_opaque())
    );
}

/// Verifies model-derived tables accept Gaman's unmanaged and opaque metadata.
#[cfg(feature = "postgres")]
#[test]
fn schema_builder_extends_tables_and_registers_opaque_entities() {
    let schema = db::schema()
        .model::<CatalogItem>()
        .extend_table("catalog_items", |table| {
            table
                .unmanaged_prefix("UNLOGGED")
                .unmanaged_suffix("TABLESPACE pg_default")
        })
        .opaque("CREATE TYPE catalog_state AS ENUM ('ready', 'complete')")
        .opaque("CREATE INDEX catalog_items_lower_slug_idx ON catalog_items ((lower(slug)))")
        .build()
        .expect("extended schema");

    let catalog = &schema.tables["catalog_items"];
    assert!(catalog.has_unmanaged_options());
    assert!(catalog.indexes.iter().any(|value| value.is_opaque()));
    assert!(schema.enums["catalog_state"].is_opaque());
}

/// Verifies schema-builder errors remain deferred and accumulate at the terminal build.
#[cfg(feature = "postgres")]
#[test]
fn schema_builder_defers_opaque_and_extension_errors() {
    let error = db::schema()
        .opaque("CREATE OR REPLACE VIEW recent_documents AS SELECT 1")
        .extend_table("missing_documents", |table| {
            table.unmanaged_prefix("UNLOGGED")
        })
        .build()
        .expect_err("invalid additions must fail at build");
    let message = error.to_string();

    assert!(message.contains("CREATE OR REPLACE"));
    assert!(message.contains("missing_documents"));
}

/// Verifies malformed unmanaged clauses fail at the terminal build on every backend.
#[test]
fn schema_builder_rejects_unsafe_unmanaged_clauses() {
    let error = db::schema()
        .model::<CatalogItem>()
        .extend_table("catalog_items", |table| {
            table.unmanaged_suffix("; DROP TABLE catalog_items")
        })
        .build()
        .expect_err("unsafe unmanaged suffix must be rejected");

    assert!(error.to_string().contains("statement terminator"));
}

/// Verifies enum-aware schema building exposes dialect validation failures.
#[test]
fn schema_builder_returns_validation_errors() {
    let error = db::schema()
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
    let table = <CatalogItem as db::schema::IntoTable>::into_table(&active_dialect());

    assert_eq!(table.indexes.len(), 1);
    assert_eq!(table.indexes[0].name, "catalog_items_slug_idx");
    assert_eq!(table.indexes[0].columns, vec!["slug"]);
    assert!(!table.indexes[0].unique);
    assert!(table.constraints.iter().any(|constraint| matches!(
        constraint,
        db::schema::Constraint::Unique { name, columns }
            if name == "catalog_items_sku_key" && columns == &vec!["sku".to_string()]
    )));
    assert_eq!(
        col(&table, "price_cents").check.as_deref(),
        Some("price_cents >= 0")
    );
}

/// Verifies common SQLx value families infer stable dialect-specific schema types.
#[test]
fn model_derive_infers_temporal_uuid_and_json_aliases() {
    let table = <InferredTypeRow as db::schema::IntoTable>::into_table(&active_dialect());

    let expected = match active_dialect() {
        db::gaman::core::Dialect::Postgres => ("uuid", "timestamptz", "jsonb", "bytea"),
        db::gaman::core::Dialect::Sqlite => ("blob", "text", "text", "blob"),
        db::gaman::core::Dialect::Mysql | db::gaman::core::Dialect::Mariadb => {
            ("binary(16)", "timestamp(6)", "json", "blob")
        }
    };

    assert_eq!(col(&table, "id").col_type, expected.0);
    assert_eq!(col(&table, "optional_id").col_type, expected.0);
    assert!(col(&table, "optional_id").nullable);
    assert_eq!(col(&table, "created_at").col_type, expected.1);
    assert_eq!(col(&table, "processed_at").col_type, expected.1);
    assert!(col(&table, "processed_at").nullable);
    assert_eq!(col(&table, "metadata").col_type, expected.2);
    assert_eq!(col(&table, "typed_metadata").col_type, expected.2);
    assert_eq!(col(&table, "app_metadata").col_type, expected.2);
    assert_eq!(col(&table, "optional_metadata").col_type, expected.2);
    assert!(col(&table, "optional_metadata").nullable);
    assert_eq!(col(&table, "bytes").col_type, expected.3);
    assert_eq!(
        col(&table, "explicit_timestamp").col_type,
        "timestamp with time zone"
    );
    assert_eq!(col(&table, "ordered_document").col_type, "json");
}

/// Verifies aliased `time` values infer their date and timestamp families.
#[cfg(feature = "time")]
#[test]
fn model_derive_infers_time_aliases() {
    let table = <InferredTimeRow as db::schema::IntoTable>::into_table(&active_dialect());
    let expected = match active_dialect() {
        db::gaman::core::Dialect::Postgres => ("timestamptz", "timestamp", "date"),
        db::gaman::core::Dialect::Sqlite => ("text", "text", "text"),
        db::gaman::core::Dialect::Mysql | db::gaman::core::Dialect::Mariadb => {
            ("timestamp(6)", "datetime(6)", "date")
        }
    };

    assert_eq!(col(&table, "occurred_at").col_type, expected.0);
    assert_eq!(col(&table, "optional_at").col_type, expected.0);
    assert!(col(&table, "optional_at").nullable);
    assert_eq!(col(&table, "local_at").col_type, expected.1);
    assert_eq!(col(&table, "event_date").col_type, expected.2);
}

/// Verifies PostgreSQL arrays derive their SQL names from SQLx element metadata.
#[cfg(feature = "postgres")]
#[test]
fn model_derive_infers_postgres_array_types() {
    let table = <InferredArrayRow as db::schema::IntoTable>::into_table(&active_dialect());

    assert_eq!(col(&table, "identifiers").col_type, "uuid[]");
    assert_eq!(col(&table, "timestamps").col_type, "timestamptz[]");
    assert_eq!(col(&table, "documents").col_type, "jsonb[]");
    assert_eq!(col(&table, "typed_documents").col_type, "jsonb[]");
    assert_eq!(col(&table, "custom_keys").col_type, "bigint[]");
    assert_eq!(col(&table, "statuses").col_type, "array_status[]");
    #[cfg(feature = "time")]
    assert_eq!(col(&table, "time_timestamps").col_type, "timestamptz[]");
}
