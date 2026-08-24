use mool as db;
use mool::Model;

#[derive(Debug, Clone, db::Model)]
#[table(name = "mutation_posts")]
struct MutationPost {
    id: i64,
    user_id: i64,
    title: String,
    published: bool,
}

#[derive(Debug, Clone, db::Model)]
#[table(name = "mutation_users")]
struct MutationUser {
    id: i64,
    email: String,
    active: bool,
}

#[derive(Debug, Clone, db::Record)]
#[table(name = "mutation_posts")]
struct MutationPatch {
    published: bool,
}

#[derive(Debug, Clone, db::Record)]
#[table(name = "mutation_users")]
struct MutationUserView {
    id: i64,
    email: String,
}

/// Verifies typed table sources drive one portable update without an explicit join API.
#[test]
fn table_source_drives_portable_update() {
    let posts = MutationPost::table();
    let users = MutationUser::table();
    let plan = db::from(&posts)
        .filter(posts.user_id.eq(users.id.clone()))
        .filter(users.active.eq(db::val(true)))
        .update(&MutationPatch { published: true })
        .set(&posts.title, users.email.clone())
        .from(&users)
        .plan()
        .expect("source-driven table update");

    assert_eq!(plan.sql, table_update_sql());
    assert_eq!(plan.prebound_count, 1);
    assert_eq!(plan.dynamic_bind_count, 1);
    assert_eq!(plan.total_bind_count, 2);
}

/// Verifies typed CTE sources retain allocation identity and source-owned assignments.
#[test]
fn cte_source_drives_portable_update() {
    let posts = MutationPost::table();
    let users = MutationUser::table();
    let active_users = db::from(&users)
        .filter(users.active.eq(db::val(true)))
        .all::<MutationUserView>()
        .cte_as("active_users");
    let active = active_users.cols();
    let plan = db::from(&posts)
        .with(&active_users)
        .filter(posts.user_id.eq(active.id.clone()))
        .update(&MutationPatch { published: true })
        .set(&posts.title, active.email.clone())
        .from(&active_users)
        .plan()
        .expect("source-driven CTE update");

    assert!(plan.sql.contains(cte_source_sql_label()));
    assert!(plan.sql.contains("__mool_source_0.email"));
    assert_eq!(plan.total_bind_count, 2);
}

/// Verifies duplicate and self-source attachments fail through structured planning errors.
#[test]
fn invalid_mutation_source_attachments_are_rejected() {
    let posts = MutationPost::table();
    let users = MutationUser::table();
    let patch = MutationPatch { published: true };

    let duplicate = db::from(&posts)
        .filter(posts.user_id.eq(users.id.clone()))
        .update(&patch)
        .from(&users)
        .from(&users)
        .plan()
        .expect_err("duplicate source");
    assert!(matches!(
        duplicate,
        db::QueryError::DuplicateMutationSource { .. }
    ));

    let target = db::from(&posts)
        .filter(posts.id.eq(db::val(1_i64)))
        .update(&patch)
        .from(&posts)
        .plan()
        .expect_err("target source");
    assert!(matches!(
        target,
        db::QueryError::MutationTargetSource { .. }
    ));
}

/// Verifies attaching a source does not bypass the mutation filter requirement.
#[test]
fn mutation_source_does_not_replace_required_filter() {
    let posts = MutationPost::table();
    let users = MutationUser::table();
    let error = db::from(&posts)
        .update(&MutationPatch { published: true })
        .from(&users)
        .plan()
        .expect_err("source-only update");

    assert!(matches!(
        error,
        db::QueryError::BindError(message) if message.contains("at least one filter")
    ));
}

#[cfg(feature = "postgres")]
/// Verifies PostgreSQL exposes native single-target `DELETE ... USING` composition.
#[test]
fn postgres_delete_using_renders_native_sql() {
    use db::backend::PostgresDeleteUsingExt;

    let posts = MutationPost::table();
    let users = MutationUser::table();
    let plan = db::from(&posts)
        .filter(posts.user_id.eq(users.id.clone()))
        .filter(users.active.eq(db::val(false)))
        .delete()
        .using(&users)
        .plan()
        .expect("PostgreSQL delete using");

    assert_eq!(
        plan.sql,
        "DELETE FROM mutation_posts USING mutation_users AS __mool_source_0 WHERE (mutation_posts.user_id = __mool_source_0.id) AND (__mool_source_0.active = $1)"
    );
    assert_eq!(plan.total_bind_count, 1);
}

#[cfg(feature = "postgres")]
fn table_update_sql() -> &'static str {
    "UPDATE mutation_posts SET published = $1, title = __mool_source_0.email FROM mutation_users AS __mool_source_0 WHERE (mutation_posts.user_id = __mool_source_0.id) AND (__mool_source_0.active = $2)"
}

#[cfg(any(feature = "postgres", feature = "sqlite"))]
fn cte_source_sql_label() -> &'static str {
    "active_users"
}

#[cfg(any(feature = "mysql", feature = "mariadb"))]
fn cte_source_sql_label() -> &'static str {
    "mutation_users"
}

#[cfg(feature = "sqlite")]
fn table_update_sql() -> &'static str {
    "UPDATE mutation_posts SET published = ?, title = __mool_source_0.email FROM mutation_users AS __mool_source_0 WHERE (mutation_posts.user_id = __mool_source_0.id) AND (__mool_source_0.active = ?)"
}

#[cfg(any(feature = "mysql", feature = "mariadb"))]
fn table_update_sql() -> &'static str {
    "UPDATE mutation_posts, mutation_users AS __mool_source_0 SET published = ?, title = __mool_source_0.email WHERE (mutation_posts.user_id = __mool_source_0.id) AND (__mool_source_0.active = ?)"
}
