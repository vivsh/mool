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
struct PostPatch {
    title: String,
    published: bool,
}

fn main() -> Result<(), db::QueryError> {
    let posts = Post::table();
    let patch = PostPatch {
        title: "Hello".to_string(),
        published: false,
    };

    let select = db::from(&posts)
        .filter(posts.published.eq(db::val(true)))
        .all::<Post>()
        .plan()?;
    let insert = db::from(&posts).insert(&patch).plan()?;
    let update = db::from(&posts)
        .filter(posts.id.eq(db::val(1_i64)))
        .update(&patch)
        .plan()?;

    assert!(select.sql.starts_with("SELECT"));
    assert!(insert.sql.starts_with("INSERT INTO posts"));
    assert!(update.sql.starts_with("UPDATE posts"));
    Ok(())
}
