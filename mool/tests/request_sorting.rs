#![cfg(any(
    feature = "postgres",
    feature = "sqlite",
    feature = "mysql",
    feature = "mariadb"
))]

use mool as db;
use mool::Model;
use serde::Deserialize;

#[derive(Debug, Clone, db::Model)]
#[table(name = "sortable_posts")]
struct SortablePost {
    #[column(primary_key)]
    id: i64,
    title: String,
    created_at: i64,
}

#[derive(Debug, Clone, db::SortKey)]
#[sort(model = SortablePost)]
enum DefaultPostSort {
    Title,
    CreatedAt,
}

#[derive(Debug, Clone, db::SortKey)]
#[sort(model = SortablePost, max_terms = 2)]
enum PostSort {
    Title,
    #[sort(name = "newest", by = created_at)]
    Newest,
    Id,
}

#[derive(Deserialize)]
struct SortQuery {
    sort: Option<db::Sort<PostSort>>,
    page: Option<u32>,
}

/// Verifies parsed request terms preserve ascending and descending precedence in SQL.
#[test]
fn request_sort_renders_typed_ordering_in_request_order() {
    let posts = SortablePost::table();
    let sort = serde_json::from_str::<db::Sort<PostSort>>(r#""-newest,title""#)
        .expect("scalar request sort");
    let plan = db::from(&posts)
        .sort_with(&sort)
        .all::<SortablePost>()
        .plan()
        .expect("sortable request plan");

    assert_eq!(
        plan.sql,
        "SELECT sortable_posts.id, sortable_posts.title, sortable_posts.created_at FROM sortable_posts ORDER BY sortable_posts.created_at DESC, sortable_posts.title ASC"
    );
    assert_eq!(sort.len(), 2);
}

/// Verifies an absent request sort leaves a query without an ORDER BY clause.
#[test]
fn empty_request_sort_leaves_query_unordered() {
    let posts = SortablePost::table();
    let sort = db::Sort::<PostSort>::default();
    let plan = db::from(&posts)
        .sort_with(&sort)
        .all::<SortablePost>()
        .plan()
        .expect("unordered request plan");

    assert!(!plan.sql.contains(" ORDER BY "));
    assert!(sort.is_empty());
}

/// Verifies direct parsing rejects malformed, unknown, duplicate, and over-limit request terms.
#[test]
fn request_sort_rejects_invalid_terms() {
    assert!(matches!(
        db::Sort::<PostSort>::parse(""),
        Err(db::SortParseError::EmptyTerm { .. })
    ));
    assert!(matches!(
        db::Sort::<PostSort>::parse("unknown"),
        Err(db::SortParseError::UnknownKey { .. })
    ));
    assert!(matches!(
        db::Sort::<PostSort>::parse("title,-title"),
        Err(db::SortParseError::DuplicateKey { .. })
    ));
    assert!(matches!(
        db::Sort::<PostSort>::parse("title,newest,id"),
        Err(db::SortParseError::TooManyTerms { max: 2 })
    ));
}

/// Verifies the default derive cap rejects a second otherwise valid request term.
#[test]
fn default_request_sort_cap_is_one_term() {
    assert!(matches!(
        db::Sort::<DefaultPostSort>::parse("title,created_at"),
        Err(db::SortParseError::TooManyTerms { max: 1 })
    ));
}

/// Verifies serde rejects an object because Sort represents one scalar value.
#[test]
fn request_sort_deserialization_is_scalar() {
    assert!(serde_json::from_str::<db::Sort<PostSort>>(r#"{"sort":"title"}"#).is_err());
}

/// Verifies scalar request sorting composes with sibling endpoint query fields.
#[test]
fn request_sort_deserializes_inside_query_dto() {
    let query = serde_json::from_str::<SortQuery>(r#"{"sort":"-newest","page":2}"#)
        .expect("composable sort query");

    assert_eq!(query.page, Some(2));
    assert_eq!(query.sort.expect("sort").len(), 1);
}
