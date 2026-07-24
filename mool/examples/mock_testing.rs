#[cfg(any(debug_assertions, feature = "mock"))]
use mool as db;

#[cfg(any(debug_assertions, feature = "mock"))]
use db::DbSession;

#[cfg(any(debug_assertions, feature = "mock"))]
#[tokio::main]
async fn main() -> Result<(), db::DbError> {
    let mut session = db::mock::MockDbSession::new();
    session.plan_execute_ok("INSERT INTO posts (title) VALUES (?)", 1);

    let rows = session
        .execute(db::Statement::raw("INSERT INTO posts (title) VALUES (?)"))
        .await?;

    assert_eq!(rows, 1);
    assert_eq!(session.recorded.len(), 1);
    Ok(())
}

#[cfg(not(any(debug_assertions, feature = "mock")))]
fn main() {}
