use mool as db;
use mool::DbSession;

fn statement(sql: &str) -> db::Statement {
    db::query(sql)
        .to_statement()
        .expect("static transaction SQL has no bindings")
}

async fn setup(pool: db::backend::Pool) -> db::DbPool {
    let mut pool = db::DbPool::from_pool(pool);
    pool.execute(statement(
        "CREATE TABLE mool_transactions (id BIGINT PRIMARY KEY, name VARCHAR(100) NOT NULL)",
    ))
    .await
    .expect("create transaction table");
    pool
}

async fn count(pool: &mut db::DbPool) -> i64 {
    pool.fetch_scalar(statement("SELECT COUNT(*) FROM mool_transactions"))
        .await
        .expect("count transaction rows")
}

/// Verifies an explicitly committed transaction persists its writes.
#[db::sqlx::test]
#[ignore = "run through scripts/integration-tests.sh"]
async fn selected_backend_transaction_commits(pool: db::backend::Pool) {
    let mut pool = setup(pool).await;
    let mut transaction = pool.begin().await.expect("begin transaction");
    transaction
        .execute(statement(
            "INSERT INTO mool_transactions (id, name) VALUES (1, 'committed')",
        ))
        .await
        .expect("insert committed row");
    transaction.commit().await.expect("commit transaction");

    assert_eq!(count(&mut pool).await, 1);
}

/// Verifies an explicitly rolled-back transaction discards its writes.
#[db::sqlx::test]
#[ignore = "run through scripts/integration-tests.sh"]
async fn selected_backend_transaction_rolls_back_error(pool: db::backend::Pool) {
    let mut pool = setup(pool).await;
    let mut transaction = pool.begin().await.expect("begin transaction");
    transaction
        .execute(statement(
            "INSERT INTO mool_transactions (id, name) VALUES (1, 'rolled-back')",
        ))
        .await
        .expect("insert rolled-back row");
    transaction.rollback().await.expect("roll back transaction");
    assert_eq!(count(&mut pool).await, 0);
}

/// Verifies nested transactions use savepoints and preserve outer writes.
#[db::sqlx::test]
#[ignore = "run through scripts/integration-tests.sh"]
async fn selected_backend_nested_transaction_rolls_back(pool: db::backend::Pool) {
    let mut pool = setup(pool).await;
    let mut outer = pool.begin().await.expect("begin outer transaction");
    outer
        .execute(statement(
            "INSERT INTO mool_transactions (id, name) VALUES (1, 'outer')",
        ))
        .await
        .expect("insert outer row");
    let mut nested = outer.begin_nested().await.expect("begin savepoint");
    nested
        .execute(statement(
            "INSERT INTO mool_transactions (id, name) VALUES (2, 'nested')",
        ))
        .await
        .expect("insert nested row");
    nested.rollback().await.expect("roll back savepoint");
    outer.commit().await.expect("commit outer transaction");

    assert_eq!(count(&mut pool).await, 1);
}

/// Verifies dropping an unfinished transaction rolls back its writes.
#[db::sqlx::test]
#[ignore = "run through scripts/integration-tests.sh"]
async fn selected_backend_dropped_transaction_rolls_back(pool: db::backend::Pool) {
    let mut pool = setup(pool).await;
    {
        let mut transaction = pool.begin().await.expect("begin transaction");
        transaction
            .execute(statement(
                "INSERT INTO mool_transactions (id, name) VALUES (1, 'dropped')",
            ))
            .await
            .expect("insert uncommitted row");
    }

    assert_eq!(count(&mut pool).await, 0);
}
