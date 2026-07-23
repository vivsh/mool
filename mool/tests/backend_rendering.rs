use mool as db;
use mool::Model;
use mool::mock::{DbCallKind, MockDbSession, PlannedCall, PlannedResponse, StatementMatcher};
use mool::queries::ParamSource;

#[cfg(any(feature = "postgres", feature = "sqlite"))]
use mool::backend::IgnoreConflictsExt;
#[cfg(any(feature = "mysql", feature = "mariadb"))]
use mool::backend::IgnoreErrorsExt;
#[cfg(any(feature = "postgres", feature = "mysql"))]
use mool::backend::LockWaitExt;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
use mool::backend::ReturningExt;
#[cfg(any(feature = "postgres", feature = "mysql", feature = "mariadb"))]
use mool::backend::RowLockExt;
#[cfg(feature = "postgres")]
use mool::backend::{DistinctOnExt, PostgresUnnestExt};

#[derive(Debug, Clone, db::Model)]
#[table(name = "backend_posts")]
struct BackendPost {
    id: i64,
    title: String,
    published: bool,
}

#[cfg(any(feature = "postgres", feature = "sqlite"))]
#[derive(Debug, Clone, db::Record)]
#[table(name = "backend_posts")]
struct BackendPostSummary {
    id: i64,
    title: String,
}

#[derive(Debug, Clone, db::Record)]
struct BackendPostPatch {
    title: String,
    published: bool,
}

#[derive(Debug, Clone, db::Model)]
#[table(
    name = "backend_memberships",
    primary_key(columns = ["tenant_id", "user_id"])
)]
struct BackendMembership {
    tenant_id: i64,
    user_id: i64,
    role: String,
}

#[cfg(feature = "postgres")]
const REPEATED_VAR_SQL: &str = "SELECT backend_post.id, backend_post.title, backend_post.published FROM backend_posts backend_post WHERE ((backend_post.id = $1) OR (backend_post.id > $1))";
#[cfg(any(feature = "sqlite", feature = "mysql", feature = "mariadb"))]
const REPEATED_VAR_SQL: &str = "SELECT backend_post.id, backend_post.title, backend_post.published FROM backend_posts backend_post WHERE ((backend_post.id = ?) OR (backend_post.id > ?))";

#[cfg(feature = "postgres")]
const UPSERT_SQL: &str = "INSERT INTO backend_posts (title, published) VALUES ($1, $2) ON CONFLICT (title) DO UPDATE SET published = EXCLUDED.published";
#[cfg(feature = "sqlite")]
const UPSERT_SQL: &str = "INSERT INTO backend_posts (title, published) VALUES (?, ?) ON CONFLICT (title) DO UPDATE SET published = EXCLUDED.published";
#[cfg(any(feature = "mysql", feature = "mariadb"))]
const UPSERT_SQL: &str = "INSERT INTO backend_posts (title, published) VALUES (?, ?) ON DUPLICATE KEY UPDATE published = VALUES(published)";

#[cfg(feature = "postgres")]
const RAW_SQL: &str = "SELECT * FROM backend_posts WHERE id = $1 OR id = $1";
#[cfg(any(feature = "sqlite", feature = "mysql", feature = "mariadb"))]
const RAW_SQL: &str = "SELECT * FROM backend_posts WHERE id = ? OR id = ?";

#[cfg(feature = "postgres")]
const PREDICATE_SQL: &str = "SELECT backend_post.id, backend_post.title, backend_post.published FROM backend_posts backend_post WHERE (((backend_post.id BETWEEN $1 AND $2) AND (backend_post.title IS NOT NULL)) AND backend_post.id NOT IN ($3, $4))";
#[cfg(any(feature = "sqlite", feature = "mysql", feature = "mariadb"))]
const PREDICATE_SQL: &str = "SELECT backend_post.id, backend_post.title, backend_post.published FROM backend_posts backend_post WHERE (((backend_post.id BETWEEN ? AND ?) AND (backend_post.title IS NOT NULL)) AND backend_post.id NOT IN (?, ?))";

#[cfg(feature = "postgres")]
const CAST_SQL: &str =
    "SELECT CAST((backend_post.id + $1) AS DOUBLE PRECISION) FROM backend_posts backend_post";
#[cfg(feature = "sqlite")]
const CAST_SQL: &str = "SELECT CAST((backend_post.id + ?) AS REAL) FROM backend_posts backend_post";
#[cfg(any(feature = "mysql", feature = "mariadb"))]
const CAST_SQL: &str =
    "SELECT CAST((backend_post.id + ?) AS DOUBLE) FROM backend_posts backend_post";

/// Verifies the selected backend owns placeholder reuse and bind ordering.
#[test]
fn selected_backend_renders_typed_variables() {
    let posts = BackendPost::table();
    let id = db::var::<i64>().named("id");
    let plan = db::from(&posts)
        .filter(posts.id.eq(&id).or(posts.id.gt(&id)))
        .bind(&id, 7_i64)
        .all::<BackendPost>()
        .plan()
        .expect("valid selected-backend query");

    assert_eq!(plan.sql, REPEATED_VAR_SQL);
    let parameter = plan.params.get("id").expect("named parameter");
    assert_eq!(parameter.source, ParamSource::Var);
    #[cfg(feature = "postgres")]
    assert_eq!(parameter.occurrences, vec![1, 1]);
    #[cfg(any(feature = "sqlite", feature = "mysql", feature = "mariadb"))]
    assert_eq!(parameter.occurrences, vec![1, 2]);
}

/// Verifies upsert syntax is delegated to the selected backend renderer.
#[test]
fn selected_backend_renders_upsert() {
    let posts = BackendPost::table();
    let patch = BackendPostPatch {
        title: "typed".to_string(),
        published: true,
    };
    let rows = [patch];
    let plan = db::from(&posts)
        .batch_upsert(&rows, [&posts.title])
        .plan()
        .expect("valid selected-backend upsert");

    assert_eq!(plan.sql, UPSERT_SQL);
    assert_eq!(plan.total_bind_count, 2);
}

/// Verifies explicit batch sizing produces inspectable plans with stable row ranges.
#[test]
fn selected_backend_exposes_ordered_batch_plans() {
    let posts = BackendPost::table();
    let rows = [
        BackendPostPatch {
            title: "one".to_string(),
            published: true,
        },
        BackendPostPatch {
            title: "two".to_string(),
            published: false,
        },
        BackendPostPatch {
            title: "three".to_string(),
            published: true,
        },
    ];
    let operation = db::from(&posts).batch_insert(&rows).batch_size(2);
    let plans = operation.plans().expect("valid split batch plans");

    assert_eq!(plans.statements().len(), 2);
    assert_eq!(plans.statements()[0].rows(), 0..2);
    assert_eq!(plans.statements()[0].plan().total_bind_count, 4);
    assert_eq!(plans.statements()[1].rows(), 2..3);
    assert_eq!(plans.statements()[1].plan().total_bind_count, 2);
    assert_eq!(
        operation
            .plan()
            .expect_err("split operation has no single plan")
            .to_string(),
        "batch operation requires 2 SQL statements; use plans() to inspect them"
    );
}

/// Verifies incompatible and zero-sized batch policies fail before execution.
#[test]
fn selected_backend_rejects_invalid_batch_policies() {
    let posts = BackendPost::table();
    let rows = [BackendPostPatch {
        title: "one".to_string(),
        published: true,
    }];

    assert_eq!(db::backend::max_batch_rows(0), None);
    assert_eq!(
        db::backend::max_batch_rows(db::backend::PARAMETER_LIMIT + 1),
        None
    );
    assert_eq!(
        db::from(&posts)
            .batch_insert(&rows)
            .batch_size(0)
            .plans()
            .expect_err("zero batch size")
            .to_string(),
        "batch size must be greater than zero, got 0"
    );
    assert!(
        db::from(&posts)
            .batch_insert(&rows)
            .batch_size(1)
            .single_statement()
            .plans()
            .expect_err("conflicting batch policies")
            .to_string()
            .contains("batch_size")
    );
}

/// Verifies targeted conflict ignoring cannot silently become untargeted.
#[cfg(any(feature = "postgres", feature = "sqlite"))]
#[test]
fn selected_backend_rejects_empty_conflict_target() {
    let posts = BackendPost::table();
    let rows = [BackendPostPatch {
        title: "one".to_string(),
        published: true,
    }];
    let error = db::from(&posts)
        .batch_insert(&rows)
        .ignore_conflicts_on(Vec::<db::queries::ColumnRef>::new())
        .plan()
        .expect_err("empty targeted conflict policy");

    assert_eq!(
        error.to_string(),
        "bind error: ignore_conflicts_on requires at least one column"
    );
}

/// Verifies selective upserts use heterogeneous typed column sets and stable bind metadata.
#[test]
fn selected_backend_renders_selective_upsert() {
    let posts = BackendPost::table();
    let rows = [BackendPostPatch {
        title: "typed".to_string(),
        published: true,
    }];
    let plan = db::from(&posts)
        .batch_upsert(&rows, &posts.title)
        .update_only(&posts.published)
        .plan()
        .expect("valid selective upsert");

    assert_eq!(plan.sql, UPSERT_SQL);
    assert_eq!(plan.prebound_count, 2);
    assert_eq!(plan.dynamic_bind_count, 0);
}

/// Verifies exact conflict ignoring composes with a typed composite target.
#[cfg(any(feature = "postgres", feature = "sqlite"))]
#[test]
fn selected_backend_renders_targeted_conflict_ignore() {
    let posts = BackendPost::table();
    let rows = [BackendPostPatch {
        title: "typed".to_string(),
        published: true,
    }];
    let plan = db::from(&posts)
        .batch_insert(&rows)
        .ignore_conflicts_on((&posts.title, &posts.published))
        .plan()
        .expect("valid targeted conflict ignore");

    #[cfg(feature = "postgres")]
    assert_eq!(
        plan.sql,
        "INSERT INTO backend_posts (title, published) VALUES ($1, $2) ON CONFLICT (title, published) DO NOTHING"
    );
    #[cfg(feature = "sqlite")]
    assert_eq!(
        plan.sql,
        "INSERT INTO backend_posts (title, published) VALUES (?, ?) ON CONFLICT (title, published) DO NOTHING"
    );
    assert_eq!(plan.total_bind_count, 2);
}

/// Verifies MySQL-family broad error ignoring is explicit in generated SQL.
#[cfg(any(feature = "mysql", feature = "mariadb"))]
#[test]
fn selected_backend_renders_insert_ignore() {
    let posts = BackendPost::table();
    let rows = [BackendPostPatch {
        title: "typed".to_string(),
        published: true,
    }];
    let plan = db::from(&posts)
        .batch_insert(&rows)
        .ignore_errors()
        .plan()
        .expect("valid INSERT IGNORE");

    assert_eq!(
        plan.sql,
        "INSERT IGNORE INTO backend_posts (title, published) VALUES (?, ?)"
    );
    assert_eq!(plan.total_bind_count, 2);
}

/// Verifies model batch updates bind keys then selected values in row-major order.
#[test]
fn selected_backend_renders_batch_update() {
    let posts = BackendPost::table();
    let rows = [
        BackendPost {
            id: 1,
            title: "one".to_string(),
            published: true,
        },
        BackendPost {
            id: 2,
            title: "two".to_string(),
            published: false,
        },
    ];
    let plan = db::from(&posts)
        .batch_update(&rows, (&posts.title, &posts.published))
        .plan()
        .expect("valid batch update");

    #[cfg(feature = "postgres")]
    assert_eq!(
        plan.sql,
        "UPDATE backend_posts AS __mool_target SET title = __mool_input.__mool_title, published = __mool_input.__mool_published FROM (VALUES ($1, $2, $3), ($4, $5, $6)) AS __mool_input (__mool_id, __mool_title, __mool_published) WHERE __mool_target.id = __mool_input.__mool_id"
    );
    #[cfg(feature = "sqlite")]
    assert_eq!(
        plan.sql,
        "WITH __mool_input (__mool_id, __mool_title, __mool_published) AS (VALUES (?, ?, ?), (?, ?, ?)) UPDATE backend_posts AS __mool_target SET title = __mool_input.__mool_title, published = __mool_input.__mool_published FROM __mool_input WHERE __mool_target.id = __mool_input.__mool_id"
    );
    #[cfg(any(feature = "mysql", feature = "mariadb"))]
    assert_eq!(
        plan.sql,
        "UPDATE backend_posts AS __mool_target JOIN (SELECT ? AS __mool_id, ? AS __mool_title, ? AS __mool_published UNION ALL SELECT ?, ?, ?) AS __mool_input ON __mool_target.id = __mool_input.__mool_id SET title = __mool_input.__mool_title, published = __mool_input.__mool_published"
    );
    assert_eq!(plan.prebound_count, 6);
    assert_eq!(plan.total_bind_count, 6);
}

/// Verifies duplicate model keys reject the complete batch before SQL execution.
#[test]
fn selected_backend_rejects_duplicate_batch_update_keys() {
    let posts = BackendPost::table();
    let rows = [
        BackendPost {
            id: 1,
            title: "one".to_string(),
            published: true,
        },
        BackendPost {
            id: 1,
            title: "duplicate".to_string(),
            published: false,
        },
    ];
    let error = db::from(&posts)
        .batch_update(&rows, &posts.title)
        .batch_size(1)
        .plans()
        .expect_err("duplicate model keys");

    assert!(error.to_string().contains("duplicate primary key"));
}

/// Verifies a planning failure makes no session calls even when execution was requested.
#[tokio::test]
async fn selected_backend_stops_before_execution_on_duplicate_keys() {
    let posts = BackendPost::table();
    let rows = [
        BackendPost {
            id: 1,
            title: "one".to_string(),
            published: true,
        },
        BackendPost {
            id: 1,
            title: "duplicate".to_string(),
            published: false,
        },
    ];
    let mut session = MockDbSession::new();
    let error = db::from(&posts)
        .batch_update(&rows, &posts.title)
        .batch_size(1)
        .exec(&mut session)
        .await
        .expect_err("duplicate keys fail before execution");

    assert!(error.to_string().contains("duplicate primary key"));
    assert!(session.recorded.is_empty());
}

/// Verifies batch updates match every component of a composite model key.
#[test]
fn selected_backend_renders_composite_key_batch_update() {
    let memberships = BackendMembership::table();
    let rows = [BackendMembership {
        tenant_id: 10,
        user_id: 20,
        role: "owner".to_string(),
    }];
    let plan = db::from(&memberships)
        .batch_update(&rows, &memberships.role)
        .plan()
        .expect("valid composite-key batch update");

    assert!(plan.sql.contains(
        "__mool_target.tenant_id = __mool_input.__mool_tenant_id AND __mool_target.user_id = __mool_input.__mool_user_id"
    ));
    assert_eq!(plan.prebound_count, 3);
    assert_eq!(plan.total_bind_count, 3);
}

/// Verifies returning and additional scope filters compose with batch updates.
#[cfg(any(feature = "postgres", feature = "sqlite"))]
#[test]
fn selected_backend_composes_returning_filtered_batch_update() {
    let posts = BackendPost::table();
    let rows = [BackendPost {
        id: 1,
        title: "one".to_string(),
        published: true,
    }];
    let plan = db::from(&posts)
        .filter(posts.published.eq(db::val(false)))
        .returning::<BackendPostSummary>()
        .batch_update(&rows, &posts.title)
        .plan()
        .expect("valid returning filtered batch update");

    assert!(plan.sql.contains(" AND (published = "));
    assert!(plan.sql.ends_with(" RETURNING id, title"));
    assert_eq!(plan.prebound_count, 2);
    assert_eq!(plan.dynamic_bind_count, 1);
    assert_eq!(plan.total_bind_count, 3);
}

/// Verifies PostgreSQL transposes ordinary records into one array bind per column.
#[cfg(feature = "postgres")]
#[test]
fn selected_backend_renders_postgres_unnest() {
    let posts = BackendPost::table();
    let rows = [
        BackendPostPatch {
            title: "one".to_string(),
            published: true,
        },
        BackendPostPatch {
            title: "two".to_string(),
            published: false,
        },
    ];
    let plan = db::from(&posts)
        .batch_insert(&rows)
        .using_unnest()
        .ignore_conflicts_on(&posts.title)
        .plan()
        .expect("valid PostgreSQL UNNEST insert");

    assert_eq!(
        plan.sql,
        "INSERT INTO backend_posts (title, published) SELECT __mool_input.title, __mool_input.published FROM UNNEST($1, $2) AS __mool_input (title, published) ON CONFLICT (title) DO NOTHING"
    );
    assert_eq!(plan.prebound_count, 2);
    assert_eq!(plan.total_bind_count, 2);
}

/// Verifies PostgreSQL `UNNEST` composes with selective upsert and returning.
#[cfg(feature = "postgres")]
#[test]
fn selected_backend_composes_returning_unnest_upsert() {
    let posts = BackendPost::table();
    let rows = [BackendPostPatch {
        title: "one".to_string(),
        published: true,
    }];
    let plan = db::from(&posts)
        .returning::<BackendPostSummary>()
        .batch_upsert(&rows, &posts.title)
        .using_unnest()
        .update_only(&posts.published)
        .plan()
        .expect("valid returning PostgreSQL UNNEST upsert");

    assert_eq!(
        plan.sql,
        "INSERT INTO backend_posts (title, published) SELECT __mool_input.title, __mool_input.published FROM UNNEST($1, $2) AS __mool_input (title, published) ON CONFLICT (title) DO UPDATE SET published = EXCLUDED.published RETURNING id, title"
    );
    assert_eq!(plan.total_bind_count, 2);
}

/// Verifies batch execution chunks rows at the selected backend's parameter limit.
#[tokio::test]
async fn selected_backend_chunks_large_batch_execution() {
    use sqlx::Arguments as _;

    let posts = BackendPost::table();
    let chunk_rows = db::backend::max_batch_rows(2).expect("two bind columns");
    let rows = (0..=chunk_rows)
        .map(|index| BackendPostPatch {
            title: format!("post-{index}"),
            published: true,
        })
        .collect::<Vec<_>>();
    let mut session = MockDbSession::new();
    session.plan(PlannedCall {
        kind: DbCallKind::Execute,
        matcher: StatementMatcher::Predicate {
            description: "a full parameter-limit batch".to_string(),
            test: Box::new(|statement| {
                statement.arguments().len() == db::backend::PARAMETER_LIMIT / 2 * 2
            }),
        },
        response: PlannedResponse::OkU64(chunk_rows as u64),
    });
    session.plan(PlannedCall {
        kind: DbCallKind::Execute,
        matcher: StatementMatcher::Predicate {
            description: "the final one-row batch".to_string(),
            test: Box::new(|statement| statement.arguments().len() == 2),
        },
        response: PlannedResponse::OkU64(1),
    });

    let affected = db::from(&posts)
        .batch_insert(&rows)
        .exec(&mut session)
        .await
        .expect("chunked batch execution");

    assert_eq!(affected, rows.len() as u64);
    assert_eq!(session.recorded.len(), 2);
}

/// Verifies automatic sizing reserves parameters used by batch-update filters.
#[test]
fn selected_backend_batch_sizing_reserves_filter_parameters() {
    let posts = BackendPost::table();
    let rows_without_overhead = db::backend::max_batch_rows(3).expect("three row parameters");
    let rows = (0..rows_without_overhead)
        .map(|index| BackendPost {
            id: index as i64,
            title: format!("post-{index}"),
            published: false,
        })
        .collect::<Vec<_>>();
    let plans = db::from(&posts)
        .filter(posts.published.eq(db::val(false)))
        .batch_update(&rows, (&posts.title, &posts.published))
        .plans()
        .expect("filter-aware automatic batch sizing");

    assert_eq!(plans.statements().len(), 2);
    assert_eq!(plans.statements()[0].rows(), 0..rows_without_overhead - 1);
    assert_eq!(
        plans.statements()[0].plan().total_bind_count,
        db::backend::PARAMETER_LIMIT - 2
    );
    assert_eq!(
        plans.statements()[1].rows(),
        rows_without_overhead - 1..rows_without_overhead
    );
}

/// Verifies raw SQL placeholders follow the selected backend's binding model.
#[test]
fn selected_backend_resolves_raw_placeholders() {
    use sqlx::Arguments as _;

    let statement = db::query("SELECT * FROM backend_posts WHERE id = :id OR id = :id")
        .bind("id", 7_i64)
        .to_statement()
        .expect("valid selected-backend raw query");

    assert_eq!(statement.sql(), RAW_SQL);
    #[cfg(feature = "postgres")]
    assert_eq!(statement.arguments().len(), 1);
    #[cfg(any(feature = "sqlite", feature = "mysql", feature = "mariadb"))]
    assert_eq!(statement.arguments().len(), 2);
}

/// Verifies range, null, and negated-membership predicates render with stable bind order.
#[test]
fn selected_backend_renders_complete_predicates() {
    let posts = BackendPost::table();
    let predicate = posts
        .id
        .between(db::val(10_i64), db::val(20_i64))
        .and(posts.title.is_not_null())
        .and(posts.id.not_in_values([13_i64, 17_i64]));
    let plan = db::from(&posts)
        .filter(predicate)
        .all::<BackendPost>()
        .plan()
        .expect("valid selected-backend predicates");

    assert_eq!(plan.sql, PREDICATE_SQL);
    assert_eq!(plan.total_bind_count, 4);
}

/// Verifies arithmetic and typed casts use backend-safe target names.
#[test]
fn selected_backend_renders_arithmetic_casts() {
    let posts = BackendPost::table();
    let expression = db::funcs::cast::<i64, f64>(posts.id.add(db::val(1_i64)));
    let plan = db::from(&posts)
        .scalar(expression)
        .plan()
        .expect("valid typed cast");

    assert_eq!(plan.sql, CAST_SQL);
    assert_eq!(plan.total_bind_count, 1);
}

/// Verifies empty membership lists use deterministic boolean semantics instead of invalid SQL.
#[test]
fn selected_backend_renders_empty_membership_lists() {
    let posts = BackendPost::table();
    let empty: Vec<i64> = Vec::new();
    let in_plan = db::from(&posts)
        .filter(posts.id.in_values(empty.clone()))
        .all::<BackendPost>()
        .plan()
        .expect("empty IN list has false semantics");
    let not_in_plan = db::from(&posts)
        .filter(posts.id.not_in_values(empty))
        .all::<BackendPost>()
        .plan()
        .expect("empty NOT IN list has true semantics");

    assert!(in_plan.sql.ends_with(" WHERE FALSE"));
    assert!(not_in_plan.sql.ends_with(" WHERE TRUE"));
    assert_eq!(in_plan.total_bind_count, 0);
    assert_eq!(not_in_plan.total_bind_count, 0);
}

/// Verifies `DISTINCT` is part of deterministic row-selection SQL.
#[test]
fn selected_backend_renders_distinct_rows() {
    let posts = BackendPost::table();
    let plan = db::from(&posts)
        .distinct()
        .all::<BackendPost>()
        .plan()
        .expect("valid distinct query");

    assert!(plan.sql.starts_with("SELECT DISTINCT "));
}

/// Verifies row-only `DISTINCT` cannot be silently ignored by aggregate terminals.
#[test]
fn selected_backend_rejects_distinct_count() {
    let posts = BackendPost::table();
    let error = db::from(&posts)
        .distinct()
        .count()
        .plan()
        .expect_err("count cannot ignore distinct");

    assert_eq!(
        error.to_string(),
        "query modifier 'distinct' is not valid for aggregate terminals"
    );
}

/// Verifies PostgreSQL exposes typed `DISTINCT ON` rendering only in its build.
#[cfg(feature = "postgres")]
#[test]
fn selected_backend_renders_postgres_distinct_on() {
    let posts = BackendPost::table();
    let plan = db::from(&posts)
        .distinct_on(posts.title.clone())
        .order_by(posts.title.asc())
        .all::<BackendPost>()
        .plan()
        .expect("valid PostgreSQL distinct-on query");

    assert_eq!(
        plan.sql,
        "SELECT DISTINCT ON (backend_post.title) backend_post.id, backend_post.title, backend_post.published FROM backend_posts backend_post ORDER BY backend_post.title ASC"
    );
}

/// Verifies row-lock syntax is rendered only by lock-capable backend builds.
#[cfg(any(feature = "postgres", feature = "mysql"))]
#[test]
fn selected_backend_renders_row_lock_wait_policy() {
    let posts = BackendPost::table();
    let plan = db::from(&posts)
        .for_update()
        .skip_locked()
        .all::<BackendPost>()
        .plan()
        .expect("valid row lock");

    assert!(plan.sql.ends_with(" FOR UPDATE SKIP LOCKED"));
}

/// Verifies MariaDB uses its shared-lock syntax.
#[cfg(feature = "mariadb")]
#[test]
fn selected_backend_renders_mariadb_shared_lock() {
    let posts = BackendPost::table();
    let plan = db::from(&posts)
        .for_share()
        .all::<BackendPost>()
        .plan()
        .expect("valid MariaDB shared lock");

    assert!(plan.sql.ends_with(" LOCK IN SHARE MODE"));
}

/// Verifies wait modifiers cannot be planned without a row lock.
#[cfg(any(feature = "postgres", feature = "mysql"))]
#[test]
fn selected_backend_rejects_lock_wait_without_lock() {
    let posts = BackendPost::table();
    let error = db::from(&posts)
        .nowait()
        .all::<BackendPost>()
        .plan()
        .expect_err("wait modifier requires a lock");

    assert_eq!(
        error.to_string(),
        "invalid row lock: a wait modifier requires a row lock"
    );
}

/// Verifies aggregate terminals reject row locks instead of omitting them.
#[cfg(any(feature = "postgres", feature = "mysql", feature = "mariadb"))]
#[test]
fn selected_backend_rejects_row_lock_count() {
    let posts = BackendPost::table();
    let error = db::from(&posts)
        .for_update()
        .count()
        .plan()
        .expect_err("count cannot use a row lock");

    assert_eq!(
        error.to_string(),
        "query modifier 'row lock' is not valid for aggregate terminals"
    );
}

/// Verifies `RETURNING` renders only when the selected backend exports the capability trait.
#[cfg(any(feature = "postgres", feature = "sqlite"))]
#[test]
fn selected_backend_renders_returning() {
    let posts = BackendPost::table();
    let patch = BackendPostPatch {
        title: "typed".to_string(),
        published: true,
    };
    let plan = db::from(&posts)
        .returning::<BackendPostSummary>()
        .insert(&patch)
        .plan()
        .expect("valid selected-backend returning insert");

    #[cfg(feature = "postgres")]
    assert_eq!(
        plan.sql,
        "INSERT INTO backend_posts (title, published) VALUES ($1, $2) RETURNING id, title"
    );
    #[cfg(feature = "sqlite")]
    assert_eq!(
        plan.sql,
        "INSERT INTO backend_posts (title, published) VALUES (?, ?) RETURNING id, title"
    );
}
