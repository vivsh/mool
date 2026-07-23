#![cfg(feature = "sqlite")]

use mool as db;
use mool::DbSession;

async fn pool() -> db::DbPool {
    let config = db::DbConf {
        url: "sqlite::memory:".to_string(),
        min_connections: 1,
        max_connections: 1,
        lazy: false,
    };
    let mut pool = db::DbPool::from_conf(&config)
        .await
        .expect("SQLite test pool");
    pool.execute(statement(
        "CREATE TABLE tx_items (id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
    ))
    .await
    .expect("transaction test table");
    pool
}

fn statement(sql: &str) -> db::Statement {
    db::query(sql)
        .to_statement()
        .expect("static SQL has valid bindings")
}

async fn count(pool: &mut db::DbPool) -> i64 {
    pool.fetch_scalar(statement("SELECT COUNT(*) FROM tx_items"))
        .await
        .expect("count transaction rows")
}

/// Verifies the pool transaction helper commits only a successful callback.
#[tokio::test]
async fn transaction_helper_commits_success() {
    let mut pool = pool().await;
    pool.transaction(|transaction| {
        Box::pin(async move {
            transaction
                .execute(statement(
                    "INSERT INTO tx_items (id, name) VALUES (1, 'one')",
                ))
                .await?;
            Ok(())
        })
    })
    .await
    .expect("successful transaction");

    assert_eq!(count(&mut pool).await, 1);
}

/// Verifies callback errors cause an explicit rollback and preserve the callback error.
#[tokio::test]
async fn transaction_helper_rolls_back_error() {
    let mut pool = pool().await;
    let error = pool
        .transaction(|transaction| {
            Box::pin(async move {
                transaction
                    .execute(statement(
                        "INSERT INTO tx_items (id, name) VALUES (1, 'one')",
                    ))
                    .await?;
                Err::<(), _>(db::DbError::QuerySet(db::QueryError::MissingBinding(
                    "expected-test-error".to_string(),
                )))
            })
        })
        .await
        .expect_err("callback error must roll back");

    assert_eq!(error.code(), "statement_error");
    assert_eq!(count(&mut pool).await, 0);
}

/// Verifies nested transactions map to savepoints that can roll back independently.
#[tokio::test]
async fn nested_transaction_rollback_preserves_outer_work() {
    let mut pool = pool().await;
    let mut outer = pool.begin().await.expect("outer transaction");
    outer
        .execute(statement(
            "INSERT INTO tx_items (id, name) VALUES (1, 'outer')",
        ))
        .await
        .expect("outer insert");
    let mut nested = outer.begin_nested().await.expect("nested transaction");
    nested
        .execute(statement(
            "INSERT INTO tx_items (id, name) VALUES (2, 'nested')",
        ))
        .await
        .expect("nested insert");
    nested.rollback().await.expect("nested rollback");
    outer.commit().await.expect("outer commit");

    assert_eq!(count(&mut pool).await, 1);
}

/// Verifies dropping an unfinished SQLx-backed transaction rolls it back.
#[tokio::test]
async fn dropped_transaction_rolls_back() {
    let mut pool = pool().await;
    {
        let mut transaction = pool.begin().await.expect("transaction");
        transaction
            .execute(statement(
                "INSERT INTO tx_items (id, name) VALUES (1, 'dropped')",
            ))
            .await
            .expect("transaction insert");
    }

    assert_eq!(count(&mut pool).await, 0);
}
