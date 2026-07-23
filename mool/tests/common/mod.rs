#![allow(dead_code)]

use std::borrow::Cow;

use mool as db;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy)]
pub struct BindMeta {
    pub prebound: usize,
    pub dynamic: usize,
    pub total: usize,
}

impl BindMeta {
    pub const fn new(prebound: usize, dynamic: usize, total: usize) -> Self {
        Self {
            prebound,
            dynamic,
            total,
        }
    }
}

pub fn assert_plan(plan: &db::QueryPlan, expected_sql: &str, expected_binds: BindMeta) {
    assert_eq!(plan.sql, expected_sql);
    assert_plan_meta(plan, expected_binds);
}

pub fn assert_plan_meta(plan: &db::QueryPlan, expected: BindMeta) {
    assert_eq!(plan.prebound_count, expected.prebound);
    assert_eq!(plan.dynamic_bind_count, expected.dynamic);
    assert_eq!(plan.total_bind_count, expected.total);
    assert_eq!(
        plan.prebound_count + plan.dynamic_bind_count,
        plan.total_bind_count
    );
}

pub fn assert_param(
    plan: &db::QueryPlan,
    name: &str,
    source: db::queries::ParamSource,
    position: usize,
    occurrences: &[usize],
) {
    let param = plan
        .params
        .get(name)
        .unwrap_or_else(|| panic!("missing parameter {name} in {:?}", plan.params.keys()));
    assert_eq!(param.source, source);
    assert_eq!(param.position, position);
    assert_eq!(param.occurrences, occurrences);
}

pub fn assert_unsupported<T>(result: Result<T, db::QueryError>, needle: &str)
where
    T: std::fmt::Debug,
{
    let err = result.expect_err("expected query planning to fail");
    assert!(
        err.to_string().contains(needle),
        "expected error to contain {needle:?}, got {err}"
    );
}

pub fn col<'a>(table: &'a db::schema::Table, name: &str) -> &'a db::schema::Column {
    table
        .columns
        .iter()
        .find(|column| column.name == name)
        .unwrap_or_else(|| panic!("missing column {name} on {}", table.name))
}

pub fn table<'a>(schema: &'a db::schema::Schema, name: &str) -> &'a db::schema::Table {
    schema
        .tables
        .get(name)
        .unwrap_or_else(|| panic!("missing table {name}"))
}

#[derive(Debug, Clone, PartialEq, db::Model)]
#[table(name = "users")]
pub struct User {
    #[column(primary_key)]
    pub id: i64,
    #[column(unique, index)]
    pub email: String,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, db::Model)]
#[table(name = "posts")]
pub struct Post {
    pub id: i64,
    #[column(reference(target = "users.id", name = "posts_author_id_fkey"))]
    pub author_id: i64,
    pub title: String,
    pub published: bool,
    #[column(type = "timestamptz")]
    pub created_at: chrono::DateTime<chrono::Utc>,
    #[column(nullable)]
    pub subtitle: Option<String>,
}

#[derive(Debug, Clone, PartialEq, db::Model)]
#[table(name = "comments")]
pub struct Comment {
    pub id: i64,
    #[column(reference = "posts.id")]
    pub post_id: i64,
    pub body: String,
    pub flagged: bool,
}

#[derive(Debug, Clone, PartialEq, db::Model)]
#[table(name = "tags")]
pub struct Tag {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, db::Model)]
#[table(name = "post_tags")]
pub struct PostTag {
    pub id: i64,
    #[column(reference = "posts.id")]
    pub post_id: i64,
    #[column(reference = "tags.id")]
    pub tag_id: i64,
}

#[derive(Debug, Clone, db::Model)]
#[table(name = "accounts", schema = "auth")]
pub struct Account {
    #[column(primary_key)]
    pub id: i64,
    #[column(name = "email_address", type = "citext")]
    pub email: String,
    #[column(nullable)]
    pub nickname: Option<String>,
}

#[derive(Debug, Clone, db::Model)]
#[table(
    name = "memberships",
    primary_key(name = "memberships_identity", columns = ["tenant_id", "user_id"])
)]
pub struct Membership {
    pub tenant_id: i64,
    pub user_id: i64,
    pub role: String,
}

#[derive(Debug, Clone, db::Model)]
pub struct AuditLog {
    pub id: i64,
    pub message: String,
}

#[derive(Debug, Clone, db::Record)]
pub struct PostWithAuthor {
    #[column(flatten)]
    pub post: Post,
    #[column(reference(on(from = "author_id", to = "id")))]
    pub author: User,
}

#[derive(Debug, Clone, db::Record)]
#[table(name = "posts")]
pub struct PostSummary {
    pub id: i64,
    pub title: String,
}

#[derive(Debug, Clone, db::Record)]
#[table(name = "posts")]
pub struct PostPatch {
    pub title: String,
    pub published: bool,
}

#[derive(Debug, Clone, db::Record)]
#[table(name = "posts")]
pub struct PostTitlePatch {
    pub title: String,
}

#[derive(Debug, Clone, db::Record)]
#[table(name = "posts")]
pub struct PostStats {
    pub author_id: i64,
    pub post_count: i64,
    pub avg_id: f64,
}

#[derive(Debug, Clone, db::Record)]
#[table(name = "posts")]
pub struct RankedPost {
    pub id: i64,
    pub row_number: i64,
    pub rank: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, db::SqlEnum)]
#[sql_enum(rename_all = "snake_case")]
pub enum PostStatus {
    Draft,
    InReview,
    Published,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, db::SqlEnum)]
#[sql_enum(storage = "int", repr = "i16")]
pub enum PostPriority {
    #[sql_enum(code = 1)]
    Low,
    #[sql_enum(code = 2)]
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, db::SqlEnum)]
#[sql_enum(name = "native_post_status", storage = "native_postgres")]
pub enum NativePostStatus {
    #[sql_enum(value = "draft")]
    Draft,
    #[sql_enum(value = "published")]
    Published,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, db::SqlEnum)]
#[sql_enum(storage = "native_mysql", rename_all = "snake_case")]
pub enum MysqlPostStatus {
    Draft,
    Published,
}

#[derive(Debug, Clone, db::Model)]
#[table(name = "enum_posts")]
pub struct EnumPost {
    pub id: i64,
    #[column(sql_enum)]
    pub status: PostStatus,
    #[column(sql_enum)]
    pub priority: PostPriority,
}

#[derive(Debug, Clone, db::Model)]
#[table(name = "native_enum_posts")]
pub struct NativeEnumPost {
    pub id: i64,
    #[column(sql_enum)]
    pub status: NativePostStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostMeta {
    pub status: String,
}

#[derive(Debug, Clone, db::Model)]
#[table(name = "json_posts")]
pub struct JsonPost {
    pub id: i64,
    #[column(type = "jsonb")]
    pub meta: PostMeta,
}

#[cfg(feature = "postgres")]
#[derive(Debug, Clone, db::Model)]
#[table(name = "array_posts")]
pub struct ArrayPost {
    pub id: i64,
    pub tags: Vec<String>,
    pub scores: Option<Vec<i64>>,
}

#[cfg(feature = "postgres")]
#[derive(Debug, Clone, db::Filterable)]
#[filter(model = Post)]
pub struct PostFilter {
    #[filter(op = "eq")]
    pub published: Option<bool>,
    #[filter(op = "ilike", column = "title")]
    pub q: Option<String>,
    #[filter(op = "in", column = "id")]
    pub ids: Vec<i64>,
}

#[derive(Debug, Clone, db::Filterable)]
#[filter(model = EnumPost)]
pub struct EnumPostFilter {
    #[filter(op = "eq")]
    pub status: Option<PostStatus>,
    #[filter(op = "in")]
    pub priority: Vec<PostPriority>,
}

pub struct PostComments;

impl db::Backref for PostComments {
    type From = Post;
    type To = Comment;

    const NAME: &'static str = "comments";
    const CARDINALITY: db::RelationCardinality = db::RelationCardinality::Many;

    fn meta() -> db::ReferenceMeta {
        db::ReferenceMeta {
            logical_name: "comment",
            table_name: "comments",
            table_schema: None,
            columns: &[db::JoinColumn {
                from: "post.id",
                to: "post_id",
            }],
            join_type: db::JoinType::Inner,
        }
    }
}

impl db::ManyBackref for PostComments {}

pub struct PostTags;

impl db::ManyToMany for PostTags {
    type From = Post;
    type Through = PostTag;
    type To = Tag;

    const NAME: &'static str = "tags";

    fn from_through() -> db::ReferenceMeta {
        db::ReferenceMeta {
            logical_name: "post_tag",
            table_name: "post_tags",
            table_schema: None,
            columns: &[db::JoinColumn {
                from: "post.id",
                to: "post_id",
            }],
            join_type: db::JoinType::Inner,
        }
    }

    fn through_to() -> db::ReferenceMeta {
        db::ReferenceMeta {
            logical_name: "tag",
            table_name: "tags",
            table_schema: None,
            columns: &[db::JoinColumn {
                from: "tag_id",
                to: "id",
            }],
            join_type: db::JoinType::Inner,
        }
    }
}

#[derive(Clone)]
pub struct LowerTitle {
    pub title: db::queries::Column<String>,
}

impl db::DbExpression<String> for LowerTitle {
    fn args(&self) -> db::FunctionArgs {
        db::FunctionArgs::new((&self.title,))
    }

    fn render(&self, ctx: &mut db::ExprRenderCtx<'_>) -> Result<String, db::QueryError> {
        Ok(format!("LOWER({})", ctx.arg(0)?))
    }
}

#[derive(Clone)]
pub struct SearchRank;

impl db::DbFunction<f64> for SearchRank {
    fn name(&self) -> Result<Cow<'static, str>, db::QueryError> {
        Ok(Cow::Borrowed("search_rank"))
    }
}
