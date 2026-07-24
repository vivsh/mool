use mool as db;
use mool::DbSession;
use mool::Model;
#[cfg(feature = "postgres")]
use sqlx::types::Uuid;

#[cfg(any(feature = "postgres", feature = "sqlite"))]
use mool::backend::IgnoreConflictsExt;
#[cfg(any(feature = "mysql", feature = "mariadb"))]
use mool::backend::IgnoreErrorsExt;
#[cfg(feature = "postgres")]
use mool::backend::{PostgresUnnestExt, ReturningExt};

#[derive(Debug, Clone, PartialEq, db::Model)]
#[table(name = "mool_batch_posts")]
struct BatchPost {
    #[column(primary_key)]
    id: i64,
    slug: String,
    title: String,
    published: bool,
}

async fn setup(pool: db::backend::Pool) -> db::DbPool {
    let mut pool = db::DbPool::from_pool(pool);
    pool.execute(statement(
        "CREATE TABLE mool_batch_posts (id BIGINT PRIMARY KEY, slug VARCHAR(100) NOT NULL UNIQUE, title VARCHAR(100) NOT NULL, published BOOLEAN NOT NULL)",
    ))
    .await
    .expect("create batch-write table");
    pool
}

fn post(id: i64, slug: &str, title: &str) -> BatchPost {
    BatchPost {
        id,
        slug: slug.to_string(),
        title: title.to_string(),
        published: false,
    }
}

fn statement(sql: &str) -> db::Statement {
    db::query(sql)
        .to_statement()
        .expect("static batch test SQL has no bindings")
}

async fn count(pool: &mut db::DbPool) -> i64 {
    pool.fetch_scalar(statement("SELECT COUNT(*) FROM mool_batch_posts"))
        .await
        .expect("count batch rows")
}

/// Verifies batch insert, selective upsert, and model updates execute on the selected backend.
#[db::sqlx::test]
#[ignore = "run through scripts/integration-tests.sh"]
async fn selected_backend_executes_batch_write_lifecycle(pool: db::backend::Pool) {
    let mut pool = setup(pool).await;
    let table = BatchPost::table();
    let rows = [
        post(1, "one", "One"),
        post(2, "two", "Two"),
        post(3, "three", "Three"),
    ];
    let inserted = db::from(&table)
        .batch_insert(&rows)
        .batch_size(2)
        .exec(&mut pool)
        .await
        .expect("chunked batch insert");
    assert_eq!(inserted, 3);

    let upserts = [post(2, "two", "Two updated"), post(4, "four", "Four")];
    db::from(&table)
        .batch_upsert(&upserts, &table.slug)
        .update_only(&table.title)
        .exec(&mut pool)
        .await
        .expect("selective batch upsert");

    let updates = [
        post(1, "one", "One updated"),
        post(3, "three", "Three updated"),
    ];
    db::from(&table)
        .batch_update(&updates, (&table.title, &table.published))
        .exec(&mut pool)
        .await
        .expect("model batch update");

    assert_eq!(count(&mut pool).await, 4);
    let rows = db::from(&table)
        .order_by(table.id.asc())
        .all::<BatchPost>()
        .exec(&mut pool)
        .await
        .expect("read batch results");
    assert_eq!(rows[0].title, "One updated");
    assert_eq!(rows[1].title, "Two updated");
}

/// Verifies backend conflict-ignore behavior omits duplicate rows without aborting the insert.
#[db::sqlx::test]
#[ignore = "run through scripts/integration-tests.sh"]
async fn selected_backend_ignores_conflicting_batch_rows(pool: db::backend::Pool) {
    let mut pool = setup(pool).await;
    let table = BatchPost::table();
    db::from(&table)
        .batch_insert(&[post(1, "same", "Original")])
        .exec(&mut pool)
        .await
        .expect("seed conflict row");
    let rows = [post(2, "same", "Ignored"), post(3, "new", "Inserted")];

    #[cfg(any(feature = "postgres", feature = "sqlite"))]
    db::from(&table)
        .batch_insert(&rows)
        .ignore_conflicts_on(&table.slug)
        .exec(&mut pool)
        .await
        .expect("exact conflict ignore");
    #[cfg(any(feature = "mysql", feature = "mariadb"))]
    db::from(&table)
        .batch_insert(&rows)
        .ignore_errors()
        .exec(&mut pool)
        .await
        .expect("broad INSERT IGNORE");

    assert_eq!(count(&mut pool).await, 2);
}

/// Verifies separate statements can partially commit and an explicit transaction can roll back.
#[db::sqlx::test]
#[ignore = "run through scripts/integration-tests.sh"]
async fn selected_backend_batch_atomicity_is_explicit(pool: db::backend::Pool) {
    let mut pool = setup(pool).await;
    let table = BatchPost::table();
    db::from(&table)
        .batch_insert(&[post(1, "existing", "Existing")])
        .exec(&mut pool)
        .await
        .expect("seed duplicate");
    let rows = [post(2, "first", "First"), post(3, "existing", "Duplicate")];
    assert!(
        db::from(&table)
            .batch_insert(&rows)
            .batch_size(1)
            .exec(&mut pool)
            .await
            .is_err()
    );
    assert_eq!(count(&mut pool).await, 2);

    let mut transaction = pool.begin().await.expect("begin explicit transaction");
    let transactional = [
        post(4, "transactional", "First"),
        post(5, "existing", "Duplicate"),
    ];
    assert!(
        db::from(&table)
            .batch_insert(&transactional)
            .batch_size(1)
            .exec(&mut transaction)
            .await
            .is_err()
    );
    transaction.rollback().await.expect("rollback failed batch");
    assert_eq!(count(&mut pool).await, 2);
}

#[cfg(feature = "postgres")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, db::SqlEnum)]
#[sql_enum(rename_all = "snake_case")]
enum UnnestStatus {
    Draft,
    Published,
}

#[cfg(feature = "postgres")]
#[derive(Debug, Clone, PartialEq, db::Model)]
#[table(name = "mool_unnest_rows")]
struct UnnestRow {
    #[column(primary_key)]
    id: i64,
    #[column(type = "uuid")]
    external_id: Uuid,
    #[column(type = "timestamptz")]
    created_at: chrono::DateTime<chrono::Utc>,
    #[column(type = "jsonb")]
    metadata: serde_json::Value,
    subtitle: Option<String>,
    #[column(sql_enum)]
    status: UnnestStatus,
}

#[cfg(feature = "postgres")]
#[derive(Debug, Clone, db::Record)]
#[table(name = "mool_unnest_rows")]
struct UnnestInput {
    #[column(type = "uuid")]
    external_id: Uuid,
    #[column(type = "timestamptz")]
    created_at: chrono::DateTime<chrono::Utc>,
    #[column(type = "jsonb")]
    metadata: serde_json::Value,
    subtitle: Option<String>,
    #[column(sql_enum)]
    status: UnnestStatus,
}

/// Verifies PostgreSQL UNNEST supports generated arrays, defaults, enums, conflicts, and upserts.
#[cfg(feature = "postgres")]
#[db::sqlx::test]
#[ignore = "run through scripts/integration-tests.sh"]
async fn postgres_executes_generated_unnest_arrays(pool: db::backend::Pool) {
    let mut pool = db::DbPool::from_pool(pool);
    pool.execute(statement("CREATE TABLE mool_unnest_rows (id BIGSERIAL PRIMARY KEY, external_id UUID NOT NULL UNIQUE, created_at TIMESTAMPTZ NOT NULL, metadata JSONB NOT NULL, subtitle TEXT, status TEXT NOT NULL)"))
        .await
        .expect("create UNNEST table");
    let table = UnnestRow::table();
    let now = chrono::Utc::now();
    let rows = [
        UnnestInput {
            external_id: Uuid::from_u128(1),
            created_at: now,
            metadata: serde_json::json!({"row": 1}),
            subtitle: None,
            status: UnnestStatus::Draft,
        },
        UnnestInput {
            external_id: Uuid::from_u128(2),
            created_at: now,
            metadata: serde_json::json!({"row": 2}),
            subtitle: Some("two".to_string()),
            status: UnnestStatus::Published,
        },
    ];
    let returned = db::from(&table)
        .returning::<UnnestRow>()
        .batch_insert(&rows)
        .using_unnest()
        .exec(&mut pool)
        .await
        .expect("execute returning UNNEST insert");

    assert_eq!(returned.len(), rows.len());
    assert!(returned.iter().all(|row| row.id > 0));
    assert_eq!(returned[0].external_id, rows[0].external_id);
    assert_eq!(returned[0].status, UnnestStatus::Draft);
    assert_eq!(returned[1].metadata, rows[1].metadata);

    let ignored = db::from(&table)
        .returning::<UnnestRow>()
        .batch_insert(&rows[..1])
        .using_unnest()
        .ignore_conflicts_on(&table.external_id)
        .exec(&mut pool)
        .await
        .expect("execute conflict-ignoring UNNEST insert");
    assert!(ignored.is_empty());

    let changed = [UnnestInput {
        external_id: rows[0].external_id,
        created_at: now,
        metadata: serde_json::json!({"row": 1, "updated": true}),
        subtitle: Some("updated".to_string()),
        status: UnnestStatus::Published,
    }];
    let updated = db::from(&table)
        .returning::<UnnestRow>()
        .batch_upsert(&changed, &table.external_id)
        .update_only((&table.metadata, &table.subtitle, &table.status))
        .using_unnest()
        .exec(&mut pool)
        .await
        .expect("execute selective returning UNNEST upsert");
    assert_eq!(updated.len(), 1);
    assert_eq!(updated[0].subtitle.as_deref(), Some("updated"));
    assert_eq!(updated[0].status, UnnestStatus::Published);
}
