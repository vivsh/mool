use mool as db;
use mool::Model;

#[derive(Debug, Clone, db::Model)]
struct Post {
    id: i64,
}

#[derive(Debug, Clone, db::Record)]
struct PostTitle {
    title: String,
}

fn main() {
    let posts = Post::table();
    let output = db::out::<PostTitle>();
    let _ = db::from(&posts)
        .all::<PostTitle>()
        .set(&output.title, posts.id.clone());
}
