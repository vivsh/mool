use mool as db;
use mool::Model;
use mool::backend::{IgnoreConflictsExt, ReturningExt};

#[derive(Debug, Clone, db::Model)]
#[table(name = "posts")]
struct Post {
    #[column(primary_key)]
    id: i64,
    slug: String,
    title: String,
}

#[derive(Debug, Clone, db::Record)]
#[table(name = "posts")]
struct NewPost {
    slug: String,
    title: String,
}

fn main() -> Result<(), db::QueryError> {
    let posts = Post::table();
    let rows = [
        NewPost {
            slug: "typed-sql".to_string(),
            title: "Typed SQL".to_string(),
        },
        NewPost {
            slug: "migrations".to_string(),
            title: "Migration generation".to_string(),
        },
    ];

    let plan = db::from(&posts)
        .returning::<Post>()
        .batch_insert(&rows)
        .ignore_conflicts_on(&posts.slug)
        .plan()?;

    assert_eq!(
        plan.sql,
        "INSERT INTO posts (slug, title) VALUES ($1, $2), ($3, $4) ON CONFLICT (slug) DO NOTHING RETURNING id, slug, title"
    );
    assert_eq!(plan.total_bind_count, 4);
    Ok(())
}
