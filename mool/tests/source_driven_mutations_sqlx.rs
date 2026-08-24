use mool as db;
use mool::{DbSession, Model};

#[derive(Debug, Clone, db::Model)]
#[table(name = "mool_source_posts")]
struct SourcePost {
    id: i64,
    user_id: i64,
    title: String,
    published: bool,
}

#[derive(Debug, Clone, db::Model)]
#[table(name = "mool_source_users")]
struct SourceUser {
    id: i64,
    email: String,
    active: bool,
}

#[derive(Debug, Clone, db::Record)]
#[table(name = "mool_source_posts")]
struct SourcePostPatch {
    published: bool,
}

#[derive(Debug, Clone, db::Record)]
#[table(name = "mool_source_users")]
struct SourceUserView {
    id: i64,
    email: String,
}

/// Verifies each backend executes one typed CTE-driven update with correctly ordered binds.
#[sqlx::test]
#[ignore = "run through scripts/integration-tests.sh"]
async fn selected_backend_executes_source_driven_update(pool: db::backend::Pool) {
    let mut pool = db::DbPool::from_pool(pool);
    create_fixture(&mut pool).await;
    let posts = SourcePost::table();
    let users = SourceUser::table();
    let active_users = db::from(&users)
        .filter(users.active.eq(db::val(true)))
        .all::<SourceUserView>()
        .cte_as("active_users");
    let active = active_users.cols();

    let affected = db::from(&posts)
        .with(&active_users)
        .filter(posts.user_id.eq(active.id.clone()))
        .update(&SourcePostPatch { published: true })
        .set(&posts.title, active.email.clone())
        .from(&active_users)
        .exec(&mut pool)
        .await
        .expect("source-driven update");

    assert_eq!(affected, 1);
    assert_eq!(post_title(&mut pool, 1).await, "active@example.com");
    assert_eq!(post_title(&mut pool, 2).await, "draft");
}

#[cfg(feature = "postgres")]
/// Verifies PostgreSQL executes native typed `DELETE ... USING` as one statement.
#[sqlx::test]
#[ignore = "run through scripts/integration-tests.sh"]
async fn postgres_executes_delete_using(pool: db::backend::Pool) {
    use db::backend::PostgresDeleteUsingExt;

    let mut pool = db::DbPool::from_pool(pool);
    create_fixture(&mut pool).await;
    let posts = SourcePost::table();
    let users = SourceUser::table();
    let affected = db::from(&posts)
        .filter(posts.user_id.eq(users.id.clone()))
        .filter(users.active.eq(db::val(false)))
        .delete()
        .using(&users)
        .exec(&mut pool)
        .await
        .expect("delete using");

    assert_eq!(affected, 1);
    assert_eq!(post_count(&mut pool, 2).await, 0);
    assert_eq!(post_count(&mut pool, 1).await, 1);
}

/// Creates the portable source-mutation fixture through Mool statements.
async fn create_fixture(pool: &mut db::DbPool) {
    for sql in [
        "CREATE TABLE mool_source_users (id BIGINT PRIMARY KEY, email VARCHAR(100) NOT NULL, active BOOLEAN NOT NULL)",
        "CREATE TABLE mool_source_posts (id BIGINT PRIMARY KEY, user_id BIGINT NOT NULL, title VARCHAR(100) NOT NULL, published BOOLEAN NOT NULL)",
        "INSERT INTO mool_source_users (id, email, active) VALUES (10, 'active@example.com', TRUE), (20, 'inactive@example.com', FALSE)",
        "INSERT INTO mool_source_posts (id, user_id, title, published) VALUES (1, 10, 'draft', FALSE), (2, 20, 'draft', FALSE)",
    ] {
        pool.execute(statement(sql))
            .await
            .expect("fixture statement");
    }
}

/// Reads one post title after a source-driven mutation.
async fn post_title(pool: &mut db::DbPool, id: i64) -> String {
    pool.fetch_scalar(
        db::query("SELECT title FROM mool_source_posts WHERE id = :id")
            .bind("id", id)
            .to_statement()
            .expect("title query"),
    )
    .await
    .expect("post title")
}

/// Counts one post after a source-driven mutation.
#[cfg(feature = "postgres")]
async fn post_count(pool: &mut db::DbPool, id: i64) -> i64 {
    pool.fetch_scalar(
        db::query("SELECT COUNT(*) FROM mool_source_posts WHERE id = :id")
            .bind("id", id)
            .to_statement()
            .expect("count query"),
    )
    .await
    .expect("post count")
}

/// Renders trusted fixture SQL through Mool's statement boundary.
fn statement(sql: &str) -> db::Statement {
    db::query(sql).to_statement().expect("fixture SQL")
}
