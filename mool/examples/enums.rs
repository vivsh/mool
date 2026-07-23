use mool as db;
use mool::Model;

#[derive(Debug, Clone, Copy, PartialEq, Eq, db::SqlEnum)]
#[sql_enum(rename_all = "snake_case")]
enum PostStatus {
    Draft,
    InReview,
    Published,
}

#[derive(Debug, Clone, db::Model)]
#[table(name = "posts")]
struct Post {
    id: i64,
    title: String,
    #[column(sql_enum)]
    status: PostStatus,
}

fn main() -> Result<(), db::schema::SchemaLoadError> {
    let posts = Post::table();
    let plan = db::from(&posts)
        .filter(posts.status.eq(db::val(PostStatus::Published)))
        .all::<Post>()
        .plan()
        .expect("valid enum query");
    let schema = db::schema().model::<Post>().build()?;

    assert!(plan.sql.contains("post.status = $1"));
    assert!(schema.tables["posts"].constraints.iter().any(|constraint| {
        matches!(
            constraint,
            db::schema::Constraint::Check { expression, .. } if expression.contains("published")
        )
    }));
    Ok(())
}
