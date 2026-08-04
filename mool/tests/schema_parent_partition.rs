//! PostgreSQL parent-partition coverage through generic table suffix metadata.

#![cfg(feature = "postgres")]

use mool as db;

use db::gaman::core::{Answer, ClarificationKind, Decision, Dialect, OfflineError, OfflinePlanner};
use db::schema::Schema;

#[derive(Debug, Clone, db::Model)]
#[table(
    name = "partition_ledger",
    primary_key(columns = ["id", "purge_at"])
)]
struct PartitionLedger {
    id: i64,
    purge_at: chrono::DateTime<chrono::Utc>,
    url: String,
}

/// Builds the plain or parent-partitioned ledger schema used by migration tests.
fn ledger_schema(partitioned: bool) -> Schema {
    let builder = db::schema().model::<PartitionLedger>();
    let builder = if partitioned {
        builder.extend_table("partition_ledger", partition_by("purge_at"))
    } else {
        builder
    };
    builder.build().expect("ledger schema should build")
}

/// Returns a table extension that declares one PostgreSQL range-partition key.
fn partition_by(
    column: &'static str,
) -> impl FnOnce(db::schema::TableBuilder) -> db::schema::TableBuilder {
    move |table| table.unmanaged_suffix(format!("PARTITION BY RANGE ({column})"))
}

/// Resolves Gaman's explicit review requirement for unmanaged table options.
fn accepted_unmanaged_migration(planner: &OfflinePlanner, desired: Schema) -> db::gaman::Migration {
    let error = planner
        .make_migration(desired.clone(), &[])
        .expect_err("unmanaged options must require review");
    let OfflineError::NeedsInput(clarifications) = error else {
        panic!("unexpected unmanaged-option error: {error}");
    };
    let clarification = clarifications
        .into_iter()
        .find(|item| matches!(item.kind, ClarificationKind::UnmanagedTableOptions { .. }))
        .expect("unmanaged-option clarification");
    let decision = Decision {
        clarification_id: clarification.id,
        answer: Answer::AcceptRisk,
    };
    planner
        .make_migration(desired, &[decision])
        .expect("accepted parent migration should plan")
        .expect("accepted parent migration should contain metadata")
}

/// Verifies a parent suffix renders exact PostgreSQL DDL and replays without child tables.
#[test]
fn parent_partition_suffix_generates_stable_schema() {
    let desired = ledger_schema(true);
    let table = &desired.tables["partition_ledger"];
    assert_eq!(desired.tables.len(), 1);
    assert!(table.has_unmanaged_options());
    assert_eq!(table.postgres_range_partition_column(), None);
    let desired_fingerprint = table.unmanaged_options_fingerprint().map(str::to_owned);

    let planner = OfflinePlanner::new(Dialect::Postgres);
    let migration = accepted_unmanaged_migration(&planner, desired.clone());
    let sql = planner
        .sql_migrate(std::slice::from_ref(&migration))
        .expect("parent migration should render");
    assert_eq!(
        sql,
        vec![concat!(
            "CREATE TABLE \"partition_ledger\" (",
            "\"id\" bigint NOT NULL, ",
            "\"purge_at\" timestamp with time zone NOT NULL, ",
            "\"url\" text NOT NULL, ",
            "CONSTRAINT \"partition_ledger_pkey\" PRIMARY KEY (\"id\", \"purge_at\")",
            ") PARTITION BY RANGE (purge_at)"
        )]
    );

    let yaml = migration
        .to_yaml_string()
        .expect("parent migration should serialize");
    let migration =
        db::gaman::Migration::from_yaml_str(&yaml).expect("parent migration should deserialize");
    let planner = planner.from_migrations(vec![migration]);
    let replayed = planner.replay().expect("parent migration should replay");
    let replayed_table = &replayed.tables["partition_ledger"];
    assert_eq!(replayed.tables.len(), 1);
    assert!(replayed_table.has_unmanaged_options());
    assert_eq!(
        replayed_table.unmanaged_options_fingerprint(),
        desired_fingerprint.as_deref()
    );
    assert_eq!(replayed_table.postgres_range_partition_column(), None);
    assert!(
        planner
            .make_migration(desired, &[])
            .expect("unchanged parent should plan")
            .is_none()
    );
}

/// Verifies adding or changing parent partitioning never emits automatic conversion SQL.
#[test]
fn existing_plain_table_partitioning_requires_raw_migration() {
    let planner = OfflinePlanner::new(Dialect::Postgres);
    let base = planner
        .make_migration(ledger_schema(false), &[])
        .expect("plain parent should plan")
        .expect("plain parent should require creation");
    let planner = planner.from_migrations(vec![base]);
    let migration = accepted_unmanaged_migration(&planner, ledger_schema(true));
    let sql = planner
        .sql_migrate(&[migration])
        .expect("acknowledgement should render");

    assert!(sql.is_empty(), "partition conversion emitted SQL: {sql:#?}");

    let planner = OfflinePlanner::new(Dialect::Postgres);
    let base = accepted_unmanaged_migration(&planner, ledger_schema(true));
    let planner = planner.from_migrations(vec![base]);
    let changed = db::schema()
        .model::<PartitionLedger>()
        .extend_table("partition_ledger", partition_by("id"))
        .build()
        .expect("changed parent schema should build");
    let migration = accepted_unmanaged_migration(&planner, changed);
    let sql = planner
        .sql_migrate(&[migration])
        .expect("changed acknowledgement should render");

    assert!(sql.is_empty(), "partition-key change emitted SQL: {sql:#?}");
}
