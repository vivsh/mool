#![cfg(feature = "postgres")]

use mool as db;
use mool::{DbSession, Model};

#[derive(Debug, Clone, db::Model)]
#[table(name = "mool_mutation_issues")]
struct MutationIssue {
    id: i64,
    state: String,
}

#[derive(Debug, Clone, db::Record)]
struct MutationIssuePatch {
    state: String,
}

#[derive(Debug, Clone, db::Model)]
#[table(name = "mool_mutation_users")]
struct MutationUser {
    id: i64,
}

#[derive(Debug, Clone, db::Model)]
#[table(
    name = "mool_mutation_issue_developers",
    primary_key(columns = ["issue_id", "user_id"])
)]
struct MutationIssueDeveloper {
    issue_id: i64,
    user_id: i64,
}

struct MutationIssueDevelopers;

impl db::ManyToMany for MutationIssueDevelopers {
    type From = MutationIssue;
    type Through = MutationIssueDeveloper;
    type To = MutationUser;

    const NAME: &'static str = "developers";

    fn from_through() -> db::ReferenceMeta {
        db::ReferenceMeta {
            logical_name: "assignment",
            table_name: "mool_mutation_issue_developers",
            table_schema: None,
            columns: &[db::JoinColumn {
                from: "mool_mutation_issues.id",
                to: "issue_id",
            }],
            join_type: db::JoinType::Inner,
        }
    }

    fn through_to() -> db::ReferenceMeta {
        db::ReferenceMeta {
            logical_name: "developer",
            table_name: "mool_mutation_users",
            table_schema: None,
            columns: &[db::JoinColumn {
                from: "user_id",
                to: "id",
            }],
            join_type: db::JoinType::Inner,
        }
    }
}

/// Verifies PostgreSQL executes authorized many-to-many updates and deletes atomically.
#[sqlx::test]
#[ignore = "run through scripts/integration-tests.sh"]
async fn many_to_many_mutation_authorization_is_atomic(pool: db::backend::Pool) {
    let mut pool = db::DbPool::from_pool(pool);
    create_fixture(&mut pool).await;
    let issues = MutationIssue::table();

    let authorized_update = db::from(&issues)
        .filter(issues.id.eq(db::val(1_i64)))
        .filter(
            db::many_to_many::<MutationIssueDevelopers>(&issues)
                .any(|developer| developer.id.eq(db::val(10_i64))),
        )
        .update(&MutationIssuePatch {
            state: "resolved".to_string(),
        })
        .exec(&mut pool)
        .await
        .expect("authorized update");
    assert_eq!(authorized_update, 1);

    let unauthorized_update = db::from(&issues)
        .filter(issues.id.eq(db::val(2_i64)))
        .filter(
            db::many_to_many::<MutationIssueDevelopers>(&issues)
                .any(|developer| developer.id.eq(db::val(10_i64))),
        )
        .update(&MutationIssuePatch {
            state: "resolved".to_string(),
        })
        .exec(&mut pool)
        .await
        .expect("unauthorized update");
    assert_eq!(unauthorized_update, 0);
    assert_eq!(issue_state(&mut pool, 2).await, "open");

    let unauthorized_delete = db::from(&issues)
        .filter(issues.id.eq(db::val(1_i64)))
        .filter(
            db::many_to_many::<MutationIssueDevelopers>(&issues)
                .any(|developer| developer.id.eq(db::val(20_i64))),
        )
        .delete()
        .exec(&mut pool)
        .await
        .expect("unauthorized delete");
    assert_eq!(unauthorized_delete, 0);

    let authorized_delete = db::from(&issues)
        .filter(issues.id.eq(db::val(1_i64)))
        .filter(
            db::many_to_many::<MutationIssueDevelopers>(&issues)
                .any(|developer| developer.id.eq(db::val(10_i64))),
        )
        .delete()
        .exec(&mut pool)
        .await
        .expect("authorized delete");
    assert_eq!(authorized_delete, 1);
    assert_eq!(issue_count(&mut pool, 1).await, 0);
}

/// Builds the relation fixture through Mool's normal statement boundary.
async fn create_fixture(pool: &mut db::DbPool) {
    for sql in [
        "CREATE TABLE mool_mutation_issues (id BIGINT PRIMARY KEY, state TEXT NOT NULL)",
        "CREATE TABLE mool_mutation_users (id BIGINT PRIMARY KEY)",
        "CREATE TABLE mool_mutation_issue_developers (issue_id BIGINT NOT NULL, user_id BIGINT NOT NULL, PRIMARY KEY (issue_id, user_id))",
        "INSERT INTO mool_mutation_issues (id, state) VALUES (1, 'open'), (2, 'open')",
        "INSERT INTO mool_mutation_users (id) VALUES (10), (20)",
        "INSERT INTO mool_mutation_issue_developers (issue_id, user_id) VALUES (1, 10), (2, 20)",
    ] {
        pool.execute(statement(sql))
            .await
            .expect("fixture statement");
    }
}

/// Reads one issue state after a tested mutation.
async fn issue_state(pool: &mut db::DbPool, id: i64) -> String {
    pool.fetch_scalar(
        db::query("SELECT state FROM mool_mutation_issues WHERE id = :id")
            .bind("id", id)
            .to_statement()
            .expect("state query"),
    )
    .await
    .expect("issue state")
}

/// Counts one issue after a tested delete.
async fn issue_count(pool: &mut db::DbPool, id: i64) -> i64 {
    pool.fetch_scalar(
        db::query("SELECT COUNT(*) FROM mool_mutation_issues WHERE id = :id")
            .bind("id", id)
            .to_statement()
            .expect("count query"),
    )
    .await
    .expect("issue count")
}

/// Renders trusted fixture SQL through Mool's normal statement boundary.
fn statement(sql: &str) -> db::Statement {
    db::query(sql).to_statement().expect("fixture SQL")
}
