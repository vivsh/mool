mod common;

use common::{Post, PostSummary};
use mool as db;
use mool::DBSession;
use mool::Model;
use mool::mock::{DbCallKind, DummyPool, MockDBSession, PlannedCall, PlannedResponse};

fn post(id: i64, title: &str) -> Post {
    Post {
        id,
        author_id: 10,
        title: title.to_string(),
        published: true,
        created_at: chrono::Utc::now(),
        subtitle: None,
    }
}

/// Verifies planned execute/scalar/all/one/optional calls return values and record statements.
#[tokio::test]
async fn mock_session_executes_planned_calls_and_records_sql() {
    let mut session = MockDBSession::new();
    session.plan_execute_ok("INSERT INTO posts", 1);
    session.plan_fetch_scalar_ok("COUNT", 2_i64);
    session.plan(PlannedCall {
        kind: DbCallKind::FetchAll,
        sql_contains: Some("FROM posts"),
        response: PlannedResponse::OkAnyVec(Box::new(vec![post(1, "a"), post(2, "b")])),
    });
    session.plan_fetch_one_ok("LIMIT 2", post(1, "a"));
    session.plan(PlannedCall {
        kind: DbCallKind::FetchOptional,
        sql_contains: Some("LIMIT 1"),
        response: PlannedResponse::OkAnyOpt(Box::new(Some(post(1, "a")))),
    });

    assert_eq!(
        session
            .execute(db::Statement::from_str(
                "INSERT INTO posts (title) VALUES (?)"
            ))
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        session
            .fetch_scalar::<i64>(db::Statement::from_str("SELECT COUNT(*) FROM posts"))
            .await
            .unwrap(),
        2
    );
    assert_eq!(
        session
            .fetch_all::<Post>(db::Statement::from_str("SELECT * FROM posts"))
            .await
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        session
            .fetch_one::<Post>(db::Statement::from_str(
                "SELECT * FROM posts LIMIT 2 OFFSET 0",
            ))
            .await
            .unwrap()
            .id,
        1
    );
    assert_eq!(
        session
            .fetch_optional::<Post>(db::Statement::from_str(
                "SELECT * FROM posts LIMIT 1 OFFSET 0",
            ))
            .await
            .unwrap()
            .unwrap()
            .title,
        "a"
    );

    assert_eq!(session.recorded.len(), 5);
    assert_eq!(session.recorded[0].kind, DbCallKind::Execute);
    assert!(session.recorded[0].stmt.sql().contains("INSERT INTO posts"));
}

/// Verifies query terminals work through the mock session without a database.
#[tokio::test]
async fn mock_session_supports_typed_query_terminals() {
    let mut session = MockDBSession::new();
    session.plan(PlannedCall {
        kind: DbCallKind::FetchAll,
        sql_contains: Some("FROM posts"),
        response: PlannedResponse::OkAnyVec(Box::new(vec![post(1, "a")])),
    });
    session.plan_fetch_scalar_ok("COUNT", 1_i64);

    let posts = Post::table();
    let rows = db::from(&posts)
        .all::<Post>()
        .exec(&mut session)
        .await
        .unwrap();
    let total = db::from(&posts).count().exec(&mut session).await.unwrap();

    assert_eq!(rows[0].id, 1);
    assert_eq!(total, 1);
}

/// Verifies mock type mismatches are returned as unsupported errors.
#[tokio::test]
async fn mock_session_reports_type_mismatch_errors() {
    let mut session = MockDBSession::new();
    session.plan_fetch_scalar_ok("COUNT", 1_i64);

    let err = session
        .fetch_scalar::<String>(db::Statement::from_str("SELECT COUNT(*) FROM posts"))
        .await
        .unwrap_err();

    assert_eq!(err.code(), "unsupported_feature");
    assert!(err.to_string().contains("fetch_scalar type mismatch"));
}

/// Verifies relaxed mocks return errors instead of panicking on unexpected calls.
#[tokio::test]
async fn relaxed_mock_session_returns_unexpected_call_error() {
    let mut session = MockDBSession::new();
    session.strict = false;

    let err = session
        .execute(db::Statement::from_str("DELETE FROM posts"))
        .await
        .unwrap_err();

    assert_eq!(err.code(), "unsupported_feature");
    assert!(err.to_string().contains("unexpected call"));
}

/// Verifies strict mocks protect planned call ordering.
#[tokio::test]
#[should_panic(expected = "expected call Execute, got FetchScalar")]
async fn strict_mock_session_panics_on_call_order_mismatch() {
    let mut session = MockDBSession::new();
    session.plan_execute_ok("INSERT", 1);

    let _ = session
        .fetch_scalar::<i64>(db::Statement::from_str("SELECT COUNT(*)"))
        .await;
}

/// Verifies DummyPool delegates plans and exposes recorded calls.
#[tokio::test]
async fn dummy_pool_delegates_to_mock_session() {
    let mut pool = DummyPool::new();
    pool.plan(PlannedCall {
        kind: DbCallKind::FetchAll,
        sql_contains: Some("FROM posts"),
        response: PlannedResponse::OkAnyVec(Box::new(vec![PostSummary {
            id: 1,
            title: "a".to_string(),
        }])),
    });

    let rows = pool
        .fetch_all::<PostSummary>(db::Statement::from_str("SELECT id, title FROM posts"))
        .await
        .unwrap();

    assert_eq!(rows[0].title, "a");
    assert_eq!(pool.recorded().len(), 1);
    assert_eq!(pool.recorded()[0].kind, DbCallKind::FetchAll);
}
