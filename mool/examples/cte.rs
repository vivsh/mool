use mool as db;
use mool::Model;

#[derive(Debug, Clone, db::Model)]
#[table(name = "posts")]
struct Post {
    #[column(primary_key)]
    id: i64,
    title: String,
    published: bool,
}

#[derive(Debug, Clone, db::Record)]
#[table(name = "posts")]
struct PublishedPost {
    id: i64,
    title: String,
}

fn main() -> Result<(), db::QueryError> {
    let posts = Post::table();
    let published_posts = db::from(&posts)
        .filter(posts.published.eq(db::val(true)))
        .all::<PublishedPost>()
        .cte_as("published_posts");
    let published = published_posts.cols();

    let plan = db::from(&published_posts)
        .with(&published_posts)
        .filter(published.id.gt(db::val(10_i64)))
        .all::<PublishedPost>()
        .plan()?;

    assert!(plan.sql.starts_with("WITH published_posts AS"));
    assert!(plan.sql.contains("FROM published_posts WHERE"));
    Ok(())
}
