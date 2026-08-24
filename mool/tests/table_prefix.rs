pub mod common;

use mool as db;
use mool::{Model, TablePrefix};

#[derive(Debug, Clone, db::Model)]
#[table(name = "prefix_widgets")]
struct PrefixWidget {
    #[column(primary_key)]
    id: i64,
    label: String,
}

#[cfg(feature = "postgres")]
#[derive(Debug, Clone, db::Model)]
#[table(name = "schema_widgets", schema = "inventory")]
struct SchemaWidget {
    #[column(primary_key)]
    id: i64,
}

#[derive(Debug, Clone, db::Record)]
struct PrefixWidgetPatch {
    label: String,
}

#[derive(Debug, Clone, db::Record, db::ManagedRecord)]
struct PrefixWidgetSeed {
    id: i64,
    label: String,
}

#[derive(Debug, Clone, db::Model)]
#[table(name = "prefix_parents")]
struct PrefixParent {
    #[column(primary_key)]
    id: i64,
}

#[derive(Debug, Clone, db::Model)]
#[table(name = "prefix_children")]
struct PrefixChild {
    #[column(primary_key)]
    id: i64,
    #[column(reference = "prefix_parents.id")]
    parent_id: i64,
}

struct StorePrefix;

impl db::TablePrefix for StorePrefix {
    const PREFIX: &'static str = "store_";
}

struct OtherPrefix;

impl db::TablePrefix for OtherPrefix {
    const PREFIX: &'static str = "other_";
}

struct InvalidPrefix;

impl db::TablePrefix for InvalidPrefix {
    const PREFIX: &'static str = "invalid-";
}

/// Verifies exact prefix concatenation on typed handles.
#[test]
fn typed_table_uses_exact_prefix() {
    let table = StorePrefix::table::<PrefixWidget>();
    let meta = db::meta(&table);

    assert_eq!(
        StorePrefix::table_name::<PrefixWidget>(),
        "store_prefix_widgets"
    );
    assert_eq!(meta.name(), "store_prefix_widgets");
    assert_eq!(meta.schema(), None);
}

/// Verifies prefixing preserves a model's existing database schema.
#[cfg(feature = "postgres")]
#[test]
fn typed_table_preserves_schema() {
    let table = StorePrefix::table::<SchemaWidget>();
    let meta = db::meta(&table);

    assert_eq!(meta.name(), "store_schema_widgets");
    assert_eq!(meta.schema(), Some("inventory"));
}

/// Verifies a prefixed typed source can project its original model safely.
#[test]
fn prefixed_table_accepts_original_model_projection() {
    let table = StorePrefix::table::<PrefixWidget>();
    let plan = db::from(&table)
        .all::<PrefixWidget>()
        .plan()
        .expect("prefixed model projection");

    assert!(plan.sql.contains("store_prefix_widgets"));
    assert!(plan.sql.contains("store_prefix_widgets.id"));
}

/// Verifies reads and every mutation terminal retain the prefixed physical target.
#[test]
fn prefixed_table_renders_reads_and_mutations() {
    let table = StorePrefix::table::<PrefixWidget>();
    let row = PrefixWidget {
        id: 1,
        label: "one".to_string(),
    };
    let patch = PrefixWidgetPatch {
        label: "updated".to_string(),
    };

    let insert = db::from(&table).insert(&row).plan().expect("insert plan");
    let update = db::from(&table)
        .filter(table.id.eq(db::val(1_i64)))
        .update(&patch)
        .plan()
        .expect("update plan");
    let delete = db::from(&table)
        .filter(table.id.eq(db::val(1_i64)))
        .delete()
        .plan()
        .expect("delete plan");

    for sql in [&insert.sql, &update.sql, &delete.sql] {
        assert!(sql.contains("store_prefix_widgets"), "{sql}");
    }
}

/// Verifies `NoPrefix` is identical to the existing model table API.
#[test]
fn no_prefix_matches_model_table_behavior() {
    let direct = db::from(PrefixWidget::table())
        .all::<PrefixWidget>()
        .plan()
        .expect("direct model plan");
    let identity = db::from(db::NoPrefix::table::<PrefixWidget>())
        .all::<PrefixWidget>()
        .plan()
        .expect("identity-prefix plan");

    assert_eq!(direct.sql, identity.sql);
    assert_eq!(db::NoPrefix::table_name::<PrefixWidget>(), "prefix_widgets");
}

/// Verifies one model can be bound independently under multiple prefixes.
#[test]
fn model_supports_multiple_independent_prefixes() {
    let store = db::from(StorePrefix::table::<PrefixWidget>())
        .all::<PrefixWidget>()
        .plan()
        .expect("store plan");
    let other = db::from(OtherPrefix::table::<PrefixWidget>())
        .all::<PrefixWidget>()
        .plan()
        .expect("other plan");

    assert!(store.sql.contains("store_prefix_widgets"));
    assert!(other.sql.contains("other_prefix_widgets"));
    assert_ne!(store.sql, other.sql);
}

/// Verifies an invalid combined identifier is rejected during query planning.
#[test]
fn invalid_prefix_uses_existing_query_validation() {
    let error = db::from(InvalidPrefix::table::<PrefixWidget>())
        .all::<PrefixWidget>()
        .plan()
        .expect_err("invalid prefixed identifier");

    assert!(error.to_string().contains("invalid-prefix_widgets"));
}

/// Verifies an invalid combined identifier is rejected by schema validation.
#[test]
fn invalid_prefix_uses_existing_schema_validation() {
    let error = db::schema()
        .model_with::<InvalidPrefix, PrefixWidget>()
        .build()
        .expect_err("invalid prefixed schema identifier");

    assert!(error.to_string().contains("invalid-prefix_widgets"));
}

/// Verifies a raw renamed table cannot masquerade as a typed model binding.
#[test]
fn raw_renamed_table_does_not_gain_model_identity() {
    let raw = db::queries::__private::table("store_prefix_widgets");
    let error = db::from(&raw)
        .all::<PrefixWidget>()
        .plan()
        .expect_err("raw table must not project a differently named model");

    assert!(error.to_string().contains("prefix_widgets"));
}

/// Verifies prefixed schema registration preserves model metadata and supports managed rows.
#[test]
fn schema_builder_registers_prefixed_model_and_managed_rows() {
    let schema = db::schema()
        .model_with::<StorePrefix, PrefixWidget>()
        .managed_rows_with::<StorePrefix, PrefixWidget>([PrefixWidgetSeed {
            id: 1,
            label: "seed".to_string(),
        }])
        .build()
        .expect("prefixed model schema");
    let table = schema
        .tables
        .values()
        .find(|table| table.name == "store_prefix_widgets")
        .expect("prefixed table");

    assert_eq!(table.schema, None);
    assert_eq!(table.columns.len(), 2);
}

/// Verifies identity-prefix schema registration matches the existing model method.
#[test]
fn no_prefix_schema_matches_existing_registration() {
    let direct = db::schema()
        .model::<PrefixWidget>()
        .build()
        .expect("direct schema");
    let identity = db::schema()
        .model_with::<db::NoPrefix, PrefixWidget>()
        .build()
        .expect("identity schema");

    assert_eq!(direct, identity);
}

/// Verifies prefixing a schema root does not rewrite declared foreign-key targets.
#[test]
fn prefixed_schema_keeps_foreign_key_targets_unprefixed() {
    let schema = db::schema()
        .model::<PrefixParent>()
        .model_with::<StorePrefix, PrefixChild>()
        .build()
        .expect("root-only prefixed schema");
    let child = schema
        .tables
        .values()
        .find(|table| table.name == "store_prefix_children")
        .expect("prefixed child table");

    assert_eq!(child.foreign_keys[0].to_table, "prefix_parents");
}

/// Verifies projection-generated joins retain their declared unprefixed targets.
#[cfg(feature = "postgres")]
#[test]
fn prefixed_projection_keeps_join_target_unprefixed() {
    let posts = StorePrefix::table::<common::Post>();
    let plan = db::from(&posts)
        .all::<common::PostWithAuthor>()
        .plan()
        .expect("prefixed projection plan");

    assert!(plan.sql.contains("FROM store_posts"));
    assert!(plan.sql.contains(" JOIN users author ON "));
}

/// Verifies Gaman recognizes a prefix change as a table-rename candidate.
#[cfg(feature = "postgres")]
#[test]
fn prefixed_schema_produces_existing_gaman_rename_flow() {
    use db::gaman::{OfflineError, OfflinePlanner};
    use gaman_core::clarifier::{Answer, ClarificationKind, Decision};
    use gaman_core::schema::Operation;

    let original = db::schema()
        .model::<PrefixWidget>()
        .build()
        .expect("original schema");
    let initial = OfflinePlanner::new(db::gaman::core::Dialect::Postgres)
        .make_migration(original, &[])
        .expect("initial planning")
        .expect("initial migration");
    let desired = db::schema()
        .model_with::<StorePrefix, PrefixWidget>()
        .build()
        .expect("prefixed schema");
    let planner =
        OfflinePlanner::new(db::gaman::core::Dialect::Postgres).from_migrations(vec![initial]);
    let error = planner
        .make_migration(desired.clone(), &[])
        .expect_err("rename clarification");
    let OfflineError::NeedsInput(clarifications) = error else {
        panic!("expected rename clarification");
    };
    let clarification = clarifications
        .iter()
        .find(|item| matches!(item.kind, ClarificationKind::RenameTable { .. }))
        .expect("table rename clarification");
    let candidate = match &clarification.kind {
        ClarificationKind::RenameTable { candidates, .. } => {
            candidates.first().expect("rename candidate").clone()
        }
        _ => return,
    };
    let migration = planner
        .make_migration(
            desired,
            &[Decision {
                clarification_id: clarification.id.clone(),
                answer: Answer::RenameTo(candidate),
            }],
        )
        .expect("resolved rename")
        .expect("rename migration");

    assert!(migration.operations.iter().any(|operation| matches!(
        operation,
        Operation::RenameTable { old_name, new_name }
            if old_name == "prefix_widgets" && new_name == "store_prefix_widgets"
    )));
}
