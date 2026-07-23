use mool as db;
use mool::Model;

#[derive(Debug, Clone, db::Model)]
#[table(name = "users")]
pub struct User {
    pub id: i64,
    pub email: String,
}

#[derive(Debug, Clone, db::Model)]
#[table(name = "posts")]
pub struct Post {
    pub id: i64,
    #[column(reference = "users.id")]
    pub author_id: i64,
    pub title: String,
}

#[derive(Debug, Clone, db::Record)]
pub struct PostWithAuthor {
    #[column(flatten)]
    pub post: Post,
    #[column(reference(on(from = "author_id", to = "id")))]
    pub author: User,
}

fn main() -> Result<(), db::QueryError> {
    let posts = Post::table();
    let plan = db::from(&posts).all::<PostWithAuthor>().plan()?;

    assert!(plan.sql.contains("JOIN users author"));
    Ok(())
}
