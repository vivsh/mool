mod common;

use common::{
    BindMeta, EnumPost, EnumPostFilter, JsonPost, LowerTitle, MysqlPostStatus, NativeEnumPost,
    Post, PostComments, PostPatch, PostPriority, PostStats, PostStatus, PostSummary, PostTags,
    PostTitlePatch, PostWithAuthor, RankedPost, SearchRank, User, assert_param, assert_plan,
    assert_unsupported, col,
};
use mool as db;
use mool::Model;
use mool::SqlEnum;
use mool::queries::{Dialect, ParamSource};

/// Verifies the core read renderer covers projections, filters, grouping, ordering, and bind metadata.
#[test]
fn read_golden_query_covers_projection_filters_grouping_and_binds() {
    let post = Post::table();
    let out = db::out::<PostStats>();
    let plan = db::from(&post)
        .filter(post.published.eq(db::val(true)))
        .filter(post.title.like(db::val("%mool%".to_string())))
        .filter(post.id.in_values([1_i64, 2_i64, 3_i64]))
        .group_by(post.author_id.clone())
        .having(db::funcs::count(post.id.clone()).gt(db::val(1_i64)))
        .order_by(post.author_id.asc())
        .all::<PostStats>()
        .set(&out.author_id, post.author_id.clone())
        .set(&out.post_count, db::funcs::count(post.id.clone()))
        .set(
            &out.avg_id,
            db::funcs::coalesce(db::funcs::avg(post.id.clone()), db::val(0.0_f64)),
        )
        .plan(Dialect::Postgres)
        .unwrap();

    assert_plan(
        &plan,
        "SELECT post.author_id AS author_id, COUNT(post.id) AS post_count, COALESCE(AVG(post.id), $1) AS avg_id FROM posts post WHERE (post.published = $2) AND (post.title LIKE $3) AND post.id IN ($4, $5, $6) GROUP BY post.author_id HAVING (COUNT(post.id) > $7) ORDER BY post.author_id ASC",
        BindMeta::new(0, 7, 7),
    );
    assert_param(&plan, "__typed_1", ParamSource::Val, 1, &[1]);
    assert_param(&plan, "__typed_7", ParamSource::Val, 7, &[7]);
    assert!(plan.result_type.unwrap().contains("PostStats"));
}

/// Verifies read terminals and window expressions render distinct SQL shapes.
#[test]
fn read_terminal_golden_queries_cover_limits_aggregates_and_windows() {
    let post = Post::table();

    assert_plan(
        &db::from(&post)
            .one::<Post>()
            .plan(Dialect::Postgres)
            .unwrap(),
        "SELECT post.id, post.author_id, post.title, post.published, post.created_at, post.subtitle FROM posts post LIMIT 2 OFFSET 0",
        BindMeta::new(0, 0, 0),
    );
    assert_plan(
        &db::from(&post)
            .first::<Post>()
            .plan(Dialect::Postgres)
            .unwrap(),
        "SELECT post.id, post.author_id, post.title, post.published, post.created_at, post.subtitle FROM posts post LIMIT 1 OFFSET 0",
        BindMeta::new(0, 0, 0),
    );
    assert_plan(
        &db::from(&post)
            .filter(post.published.eq(db::val(true)))
            .count()
            .plan(Dialect::Postgres)
            .unwrap(),
        "SELECT COUNT(*) FROM posts post WHERE (post.published = $1)",
        BindMeta::new(0, 1, 1),
    );
    assert_plan(
        &db::from(&post)
            .filter(post.published.eq(db::val(true)))
            .exists()
            .plan(Dialect::Postgres)
            .unwrap(),
        "SELECT EXISTS(SELECT 1 FROM posts post WHERE (post.published = $1))",
        BindMeta::new(0, 1, 1),
    );
    assert_plan(
        &db::from(&post)
            .scalar(db::funcs::max(post.id.clone()))
            .plan(Dialect::Postgres)
            .unwrap(),
        "SELECT MAX(post.id) FROM posts post",
        BindMeta::new(0, 0, 0),
    );

    let out = db::out::<RankedPost>();
    let window = db::funcs::window()
        .partition_by(post.author_id.clone())
        .order_by(post.id.asc());
    assert_plan(
        &db::from(&post)
            .all::<RankedPost>()
            .set(&out.id, post.id.clone())
            .set(
                &out.row_number,
                db::funcs::row_number().over(window.clone()),
            )
            .set(&out.rank, db::funcs::rank().over(window))
            .plan(Dialect::Postgres)
            .unwrap(),
        "SELECT post.id AS id, ROW_NUMBER() OVER (PARTITION BY post.author_id ORDER BY post.id ASC) AS row_number, RANK() OVER (PARTITION BY post.author_id ORDER BY post.id ASC) AS rank FROM posts post",
        BindMeta::new(0, 0, 0),
    );
}

/// Verifies backend-specific placeholder behavior and typed variable parameter metadata.
#[test]
fn dialect_matrix_covers_placeholders_variable_reuse_and_unsupported_features() {
    let post = Post::table();
    let id = db::var::<i64>().named("id");

    let pg = db::from(&post)
        .filter(post.id.gte(&id).and(post.author_id.eq(&id)))
        .bind(&id, 10_i64)
        .all::<Post>()
        .plan(Dialect::Postgres)
        .unwrap();
    assert_plan(
        &pg,
        "SELECT post.id, post.author_id, post.title, post.published, post.created_at, post.subtitle FROM posts post WHERE ((post.id >= $1) AND (post.author_id = $1))",
        BindMeta::new(0, 1, 1),
    );
    assert_param(&pg, "id", ParamSource::Var, 1, &[1, 1]);

    let mysql = db::from(&post)
        .filter(post.id.gte(&id).and(post.author_id.eq(&id)))
        .bind(&id, 10_i64)
        .all::<Post>()
        .plan(Dialect::Mysql)
        .unwrap();
    assert_plan(
        &mysql,
        "SELECT post.id, post.author_id, post.title, post.published, post.created_at, post.subtitle FROM posts post WHERE ((post.id >= ?) AND (post.author_id = ?))",
        BindMeta::new(0, 2, 2),
    );
    assert_param(&mysql, "id", ParamSource::Var, 1, &[1, 2]);

    let sqlite = db::from(&post)
        .filter(post.id.eq(&id))
        .bind(&id, 10_i64)
        .all::<Post>()
        .plan(Dialect::Sqlite)
        .unwrap();
    assert_plan(
        &sqlite,
        "SELECT post.id, post.author_id, post.title, post.published, post.created_at, post.subtitle FROM posts post WHERE (post.id = ?)",
        BindMeta::new(0, 1, 1),
    );

    assert_unsupported(
        db::from(&post)
            .filter(post.title.ilike(db::val("%mool%".to_string())))
            .all::<Post>()
            .plan(Dialect::Mysql),
        "ILIKE",
    );
    assert_unsupported(
        db::from(&post)
            .filter(post.title.ilike(db::val("%mool%".to_string())))
            .all::<Post>()
            .plan(Dialect::Sqlite),
        "ILIKE",
    );
}

/// Verifies write rendering covers row payloads, expression writes, upsert dialects, and returning.
#[test]
fn write_golden_queries_cover_mutations_upserts_and_returning() {
    let post = Post::table();
    let row = Post {
        id: 1,
        author_id: 2,
        title: "hello".to_string(),
        published: true,
        created_at: chrono::Utc::now(),
        subtitle: None,
    };
    let patch = PostTitlePatch {
        title: "updated".to_string(),
    };
    let rows = vec![
        PostPatch {
            title: "a".to_string(),
            published: true,
        },
        PostPatch {
            title: "b".to_string(),
            published: false,
        },
    ];

    assert_plan(
        &db::from(&post)
            .insert(&row)
            .plan(Dialect::Postgres)
            .unwrap(),
        "INSERT INTO posts (id, author_id, title, published, created_at, subtitle) VALUES ($1, $2, $3, $4, $5, $6)",
        BindMeta::new(6, 0, 6),
    );
    assert_plan(
        &db::from(&post)
            .filter(post.id.eq(db::val(1_i64)))
            .update_using(|w| {
                w.set(
                    &post.title,
                    db::funcs::coalesce(post.title.clone(), db::val("untitled".to_string())),
                )
            })
            .plan(Dialect::Postgres)
            .unwrap(),
        "UPDATE posts SET title = COALESCE(title, $1) WHERE (id = $2)",
        BindMeta::new(0, 2, 2),
    );
    assert_plan(
        &db::from(&post)
            .filter(post.published.eq(db::val(false)))
            .delete()
            .plan(Dialect::Postgres)
            .unwrap(),
        "DELETE FROM posts WHERE (published = $1)",
        BindMeta::new(0, 1, 1),
    );
    assert_plan(
        &db::from(&post)
            .batch_insert(&rows)
            .plan(Dialect::Postgres)
            .unwrap(),
        "INSERT INTO posts (title, published) VALUES ($1, $2), ($3, $4)",
        BindMeta::new(4, 0, 4),
    );
    assert_plan(
        &db::from(&post)
            .batch_upsert(&rows, [&post.title])
            .plan(Dialect::Postgres)
            .unwrap(),
        "INSERT INTO posts (title, published) VALUES ($1, $2), ($3, $4) ON CONFLICT (title) DO UPDATE SET published = EXCLUDED.published",
        BindMeta::new(4, 0, 4),
    );
    assert_plan(
        &db::from(&post)
            .batch_upsert(&rows, [&post.title])
            .plan(Dialect::Mysql)
            .unwrap(),
        "INSERT INTO posts (title, published) VALUES (?, ?), (?, ?) ON DUPLICATE KEY UPDATE published = VALUES(published)",
        BindMeta::new(4, 0, 4),
    );
    assert_plan(
        &db::from(&post)
            .returning::<PostSummary>()
            .insert(&patch)
            .plan(Dialect::Postgres)
            .unwrap(),
        "INSERT INTO posts (title) VALUES ($1) RETURNING id, title",
        BindMeta::new(1, 0, 1),
    );
    assert_unsupported(
        db::from(&post)
            .returning::<PostSummary>()
            .insert(&patch)
            .plan(Dialect::Mysql),
        "RETURNING",
    );
}

/// Verifies table, schema-qualified table, CTE, subquery, and set-operation sources.
#[test]
fn source_golden_queries_cover_tables_ctes_subqueries_and_sets() {
    let account = common::Account::table();
    assert_plan(
        &db::from(&account)
            .all::<common::Account>()
            .plan(Dialect::Postgres)
            .unwrap(),
        "SELECT account.id, account.email_address, account.nickname FROM auth.accounts account",
        BindMeta::new(0, 0, 0),
    );

    let post = Post::table();
    let subquery = db::from(&post)
        .filter(post.published.eq(db::val(true)))
        .all::<PostSummary>()
        .subquery_as("published_posts")
        .unwrap();
    let cols = subquery.cols();
    assert_plan(
        &db::from(&subquery)
            .filter(cols.id.gt(db::val(10_i64)))
            .all::<PostSummary>()
            .plan(Dialect::Postgres)
            .unwrap(),
        "SELECT published_posts.id, published_posts.title FROM (SELECT post.id, post.title FROM posts post WHERE (post.published = $1)) published_posts WHERE (published_posts.id > $2)",
        BindMeta::new(0, 2, 2),
    );

    let cte = db::from(&post)
        .filter(post.published.eq(db::val(true)))
        .all::<PostSummary>()
        .cte_as("published_posts")
        .unwrap();
    let cols = cte.cols();
    assert_plan(
        &db::from(&cte)
            .with(&cte)
            .filter(cols.id.gt(db::val(10_i64)))
            .all::<PostSummary>()
            .plan(Dialect::Postgres)
            .unwrap(),
        "WITH published_posts AS (SELECT post.id, post.title FROM posts post WHERE (post.published = $1)) SELECT published_posts.id, published_posts.title FROM published_posts WHERE (published_posts.id > $2)",
        BindMeta::new(0, 2, 2),
    );

    let left = db::from(&post)
        .filter(post.published.eq(db::val(true)))
        .all::<PostSummary>();
    let right = db::from(&post)
        .filter(post.published.eq(db::val(false)))
        .all::<PostSummary>();
    assert_plan(
        &left.union_all(right).plan(Dialect::Postgres).unwrap(),
        "SELECT post.id, post.title FROM posts post WHERE (post.published = $1) UNION ALL SELECT post.id, post.title FROM posts post WHERE (post.published = $2)",
        BindMeta::new(0, 2, 2),
    );
}

/// Verifies relation SQL generation covers joined records, backrefs, many-to-many, and aggregates.
#[test]
fn relation_golden_queries_cover_joined_records_and_correlated_predicates() {
    let post = Post::table();

    assert_plan(
        &db::from(&post)
            .all::<PostWithAuthor>()
            .plan(Dialect::Postgres)
            .unwrap(),
        "SELECT post.id, post.author_id, post.title, post.published, post.created_at, post.subtitle, author.id, author.email, author.active FROM posts post JOIN users author ON author.id = post.author_id",
        BindMeta::new(0, 0, 0),
    );
    assert_plan(
        &db::from(&post)
            .filter(
                db::backref::<PostComments>(&post).any(|comment| comment.flagged.eq(db::val(true))),
            )
            .all::<Post>()
            .plan(Dialect::Postgres)
            .unwrap(),
        "SELECT post.id, post.author_id, post.title, post.published, post.created_at, post.subtitle FROM posts post WHERE EXISTS (SELECT 1 FROM comments comment WHERE comment.post_id = post.id AND (comment.flagged = $1))",
        BindMeta::new(0, 1, 1),
    );
    assert_plan(
        &db::from(&post)
            .filter(
                db::many_to_many::<PostTags>(&post)
                    .any(|tag| tag.name.eq(db::val("rust".to_string()))),
            )
            .all::<Post>()
            .plan(Dialect::Postgres)
            .unwrap(),
        "SELECT post.id, post.author_id, post.title, post.published, post.created_at, post.subtitle FROM posts post WHERE EXISTS (SELECT 1 FROM post_tags post_tag JOIN tags tag ON tag.id = post_tag.tag_id WHERE post_tag.post_id = post.id AND (tag.name = $1))",
        BindMeta::new(0, 1, 1),
    );
    assert_plan(
        &db::from(&post)
            .filter(
                db::backref::<PostComments>(&post)
                    .count()
                    .gt(db::val(2_i64)),
            )
            .all::<Post>()
            .plan(Dialect::Postgres)
            .unwrap(),
        "SELECT post.id, post.author_id, post.title, post.published, post.created_at, post.subtitle FROM posts post WHERE ((SELECT COUNT(*) FROM comments comment WHERE comment.post_id = post.id) > $1)",
        BindMeta::new(0, 1, 1),
    );
}

/// Verifies built-in, JSON, custom, and Postgres-only function rendering.
#[test]
fn function_golden_queries_cover_builtin_json_and_custom_extensions() {
    let post = Post::table();
    let out = db::out::<PostSummary>();
    assert_plan(
        &db::from(&post)
            .all::<PostSummary>()
            .set(&out.id, post.id.clone())
            .set(
                &out.title,
                db::funcs::case()
                    .when(post.published.eq(db::val(true)), post.title.clone())
                    .else_(db::val("draft".to_string())),
            )
            .plan(Dialect::Postgres)
            .unwrap(),
        "SELECT post.id AS id, CASE WHEN (post.published = $1) THEN post.title ELSE $2 END AS title FROM posts post",
        BindMeta::new(0, 2, 2),
    );

    let json_post = JsonPost::table();
    assert_plan(
        &db::from(&json_post)
            .scalar(db::funcs::json::text(json_post.meta.clone(), "status"))
            .plan(Dialect::Postgres)
            .unwrap(),
        "SELECT (json_post.meta #>> '{status}') FROM json_posts json_post",
        BindMeta::new(0, 0, 0),
    );

    assert_plan(
        &db::from(&post)
            .filter(db::funcs::func(SearchRank, (post.title.clone(),)).gt(db::val(0.5_f64)))
            .all::<PostSummary>()
            .set(&out.id, post.id.clone())
            .set(
                &out.title,
                db::funcs::custom(LowerTitle {
                    title: post.title.clone(),
                }),
            )
            .plan(Dialect::Postgres)
            .unwrap(),
        "SELECT post.id AS id, LOWER(post.title) AS title FROM posts post WHERE (search_rank(post.title) > $1)",
        BindMeta::new(0, 1, 1),
    );
}

/// Verifies Postgres-only array helpers are covered when the Postgres feature is active.
#[test]
#[cfg(feature = "postgres")]
fn postgres_array_helper_renders_supported_sql() {
    let post = common::ArrayPost::table();
    assert_plan(
        &db::from(&post)
            .filter(db::funcs::array::contains(
                post.tags.clone(),
                db::funcs::array::value(vec!["rust".to_string()]),
            ))
            .all::<common::ArrayPost>()
            .plan(Dialect::Postgres)
            .unwrap(),
        "SELECT array_post.id, array_post.tags, array_post.scores FROM array_posts array_post WHERE (array_post.tags @> $1)",
        BindMeta::new(0, 1, 1),
    );
}

/// Verifies enum fields participate in query rendering, filter generation, and schema metadata.
#[test]
fn enum_golden_queries_cover_typed_filters_and_schema_metadata() {
    let post = EnumPost::table();
    let filter = EnumPostFilter {
        status: Some(PostStatus::Published),
        priority: vec![PostPriority::Low, PostPriority::High],
    };
    let plan = db::from(&post)
        .filter_with(&filter)
        .all::<EnumPost>()
        .plan(Dialect::Postgres)
        .unwrap();
    assert_plan(
        &plan,
        "SELECT enum_post.id, enum_post.status, enum_post.priority FROM enum_posts enum_post WHERE (enum_post.status = $1) AND enum_post.priority IN ($2, $3)",
        BindMeta::new(0, 3, 3),
    );

    let schema = db::schema(db::Dialect::Postgres)
        .model::<EnumPost>()
        .model::<NativeEnumPost>()
        .build();
    let enum_posts = common::table(&schema, "enum_posts");
    assert_eq!(col(enum_posts, "status").col_type, "text");
    assert_eq!(col(enum_posts, "priority").col_type, "smallint");
    assert!(schema.enums.contains_key("native_post_status"));
    assert!(enum_posts.constraints.iter().any(|constraint| matches!(
        constraint,
        db::Constraint::Check { name, expression }
            if name == "ck_enum_posts_status_sql_enum"
                && expression == "status IN ('draft', 'in_review', 'published')"
    )));
    assert_eq!(
        MysqlPostStatus::sql_column_type(db::Dialect::Postgres),
        "ENUM('draft', 'published')".to_string()
    );
}

/// Verifies raw SQL placeholder resolution is covered as an explicit SQL generation path.
#[test]
fn raw_sql_golden_queries_cover_named_placeholders_and_bind_errors() {
    use sqlx::Arguments as _;

    let pg = db::query("SELECT * FROM users WHERE id = :id AND email = :email")
        .bind("id", 1_i64)
        .bind("email", "a@example.com".to_string())
        .to_statement(Dialect::Postgres)
        .unwrap();
    assert_eq!(pg.sql(), "SELECT * FROM users WHERE id = $1 AND email = $2");
    assert_eq!(pg.arguments().len(), 2);

    let mysql = db::query("SELECT * FROM users WHERE id = :id AND email = :email")
        .bind("id", 1_i64)
        .bind("email", "a@example.com".to_string())
        .to_statement(Dialect::Mysql)
        .unwrap();
    assert_eq!(
        mysql.sql(),
        "SELECT * FROM users WHERE id = ? AND email = ?"
    );
    assert_eq!(mysql.arguments().len(), 2);

    let repeated = db::query("SELECT * FROM users WHERE id = :id OR manager_id = :id")
        .bind("id", 1_i64)
        .to_statement(Dialect::Postgres)
        .unwrap();
    assert_eq!(
        repeated.sql(),
        "SELECT * FROM users WHERE id = $1 OR manager_id = $1"
    );
    assert_eq!(repeated.arguments().len(), 1);

    let missing = db::query("SELECT * FROM users WHERE id = :id")
        .to_statement(Dialect::Postgres)
        .unwrap_err();
    assert!(
        missing
            .to_string()
            .contains("placeholder 'id' not found in values map")
    );

    let unused = db::query("SELECT * FROM users")
        .bind("id", 1_i64)
        .to_statement(Dialect::Postgres)
        .unwrap_err();
    assert!(unused.to_string().contains("unused binding: id"));
}

/// Verifies planning failures cover invalid identifiers, ownership mistakes, and invalid shapes.
#[test]
fn failure_contracts_reject_invalid_queries_before_execution() {
    let post = Post::table();
    let user = User::table();
    let id = db::var::<i64>().named("id");
    let bad_name = db::var::<i64>().named("bad-name");

    assert_unsupported(
        db::from(&post)
            .filter(post.id.eq(&bad_name))
            .bind(&bad_name, 1_i64)
            .all::<Post>()
            .plan(Dialect::Postgres),
        "invalid identifier",
    );
    assert_unsupported(
        db::from(&user)
            .filter(post.id.eq(db::val(1_i64)))
            .all::<User>()
            .plan(Dialect::Postgres),
        "column belongs to",
    );
    assert_unsupported(
        db::from(&post)
            .filter(post.id.eq(&id))
            .all::<Post>()
            .plan(Dialect::Postgres),
        "missing binding for id",
    );
    assert_unsupported(
        db::from(&post)
            .filter(post.id.eq(&id))
            .bind(&id, 1_i64)
            .bind(&id, 2_i64)
            .all::<Post>()
            .plan(Dialect::Postgres),
        "duplicate binding",
    );
    assert_unsupported(
        db::from(&post)
            .filter(post.id.in_values(Vec::<i64>::new()))
            .all::<Post>()
            .plan(Dialect::Postgres),
        "IN list requires at least one value",
    );

    let empty_rows: Vec<PostPatch> = Vec::new();
    assert_unsupported(
        db::from(&post)
            .batch_insert(&empty_rows)
            .plan(Dialect::Postgres),
        "cannot insert empty list",
    );

    let cte = db::from(&post)
        .all::<PostSummary>()
        .cte_as("post_ids")
        .unwrap();
    assert_unsupported(
        db::from(&cte)
            .with(&cte)
            .filter(db::val(true).eq(db::val(true)))
            .delete()
            .plan(Dialect::Postgres),
        "CTE 'post_ids' cannot be used as a mutation target",
    );
}
