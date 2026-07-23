use mool as db;
use mool::DbSession;

#[derive(Debug, PartialEq, Eq, db::sqlx::FromRow)]
struct SmokeRow {
    id: i64,
    name: String,
}

/// Verifies Mool can create, insert, and select through the selected SQLx transport.
#[db::sqlx::test]
#[ignore = "run through scripts/integration-tests.sh"]
async fn selected_backend_sqlx_smoke(pool: db::backend::Pool) {
    let mut pool = db::DbPool::from_pool(pool);
    pool.execute(statement(
        "CREATE TABLE mool_smoke (id BIGINT PRIMARY KEY, name VARCHAR(100) NOT NULL)",
    ))
    .await
    .expect("create smoke table");
    pool.execute(
        db::query("INSERT INTO mool_smoke (id, name) VALUES (:id, :name)")
            .bind("id", 1_i64)
            .bind("name", "mool".to_string())
            .to_statement()
            .expect("smoke insert statement"),
    )
    .await
    .expect("insert smoke row");
    let row = pool
        .fetch_one::<SmokeRow>(statement("SELECT id, name FROM mool_smoke WHERE id = 1"))
        .await
        .expect("select smoke row");

    assert_eq!(
        row,
        SmokeRow {
            id: 1,
            name: "mool".to_string(),
        }
    );
}

fn statement(sql: &str) -> db::Statement {
    db::query(sql)
        .to_statement()
        .expect("static smoke SQL has no bindings")
}
