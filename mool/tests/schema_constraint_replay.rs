//! Regression coverage for model-derived constraints across migration replay.

#![cfg(feature = "postgres")]

use mool as db;

#[derive(Debug, Clone, db::Model)]
#[table(name = "constraint_parents")]
struct ConstraintParent {
    #[column(primary_key)]
    id: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, db::SqlEnum)]
#[sql_enum(rename_all = "snake_case")]
enum LedgerKind {
    Primary,
    Secondary,
}

#[derive(Debug, Clone, db::Model)]
#[table(name = "constraint_ledgers")]
struct ConstraintLedger {
    #[column(primary_key)]
    id: i64,
    #[column(reference = "constraint_parents.id")]
    parent_id: i64,
    #[column(unique_name = "constraint_ledgers_slug_key")]
    slug: String,
    #[column(check = "attempts >= 0")]
    attempts: i64,
    #[column(sql_enum)]
    kind: LedgerKind,
}

/// Verifies model constraints survive migration serialization and replay without a follow-up diff.
#[test]
fn model_constraints_survive_migration_replay() {
    let desired = db::schema()
        .model::<ConstraintParent>()
        .model::<ConstraintLedger>()
        .build()
        .expect("model schema should build");
    let planner = db::gaman::core::OfflinePlanner::new(db::gaman::core::Dialect::Postgres);
    let migration = planner
        .make_migration(desired.clone(), &[])
        .expect("initial migration should plan")
        .expect("initial migration should contain operations");
    let yaml = migration
        .to_yaml_string()
        .expect("initial migration should serialize");
    assert!(yaml.contains("constraint_ledgers_slug_key"), "{yaml}");
    assert!(yaml.contains("constraint_ledgers_attempts_check"), "{yaml}");
    assert!(
        yaml.contains("ck_constraint_ledgers_kind_sql_enum"),
        "{yaml}"
    );
    assert!(yaml.contains("constraint_ledgers_parent_id_fkey"), "{yaml}");
    let ddl = planner
        .sql_migrate(std::slice::from_ref(&migration))
        .expect("initial migration should render")
        .join("\n");
    assert!(ddl.contains("constraint_ledgers_slug_key"), "{ddl}");
    assert!(ddl.contains("ck_constraint_ledgers_kind_sql_enum"), "{ddl}");
    let migration =
        db::gaman::Migration::from_yaml_str(&yaml).expect("initial migration should deserialize");
    let planner = planner.from_migrations(vec![migration]);

    let replayed = planner.replay().expect("migration should replay");
    let table = replayed
        .tables
        .get("constraint_ledgers")
        .expect("replayed table should exist");
    let mut constraint_names = table
        .constraints
        .iter()
        .map(|constraint| constraint.name())
        .collect::<Vec<_>>();
    constraint_names.sort_unstable();
    assert_eq!(
        constraint_names,
        vec![
            "ck_constraint_ledgers_kind_sql_enum",
            "constraint_ledgers_attempts_check",
            "constraint_ledgers_slug_key",
        ],
        "{table:#?}"
    );
    assert_eq!(
        table.primary_key.as_ref().map(|key| key.name.as_str()),
        Some("constraint_ledgers_pkey")
    );
    assert_eq!(table.foreign_keys.len(), 1, "{table:#?}");
    assert_eq!(
        table.foreign_keys[0].name,
        "constraint_ledgers_parent_id_fkey"
    );

    let follow_up = planner
        .make_migration(desired, &[])
        .expect("follow-up migration should plan");
    assert!(
        follow_up.is_none(),
        "unchanged constraints generated an extra migration: {follow_up:#?}"
    );
}
