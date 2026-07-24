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

/// Verifies an explicitly committed SQLite transaction persists its writes.
#[tokio::test]
async fn explicit_transaction_commit_persists_writes() {
    let mut pool = pool().await;
    let mut transaction = pool.begin().await.expect("begin transaction");
    transaction
        .execute(statement(
            "INSERT INTO tx_items (id, name) VALUES (1, 'one')",
        ))
        .await
        .expect("insert row");
    transaction.commit().await.expect("commit transaction");

    assert_eq!(count(&mut pool).await, 1);
}

/// Verifies an explicit SQLite rollback discards prior writes.
#[tokio::test]
async fn explicit_transaction_rollback_discards_writes() {
    let mut pool = pool().await;
    let mut transaction = pool.begin().await.expect("begin transaction");
    transaction
        .execute(statement(
            "INSERT INTO tx_items (id, name) VALUES (1, 'one')",
        ))
        .await
        .expect("insert row");
    transaction.rollback().await.expect("rollback transaction");
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
