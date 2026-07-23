use mool as db;
use mool::Model;

#[derive(Debug, Clone, db::Model)]
struct Post {
    id: i64,
}

fn main() {
    let posts = Post::table();
    let _ = posts.id.eq(db::val("wrong".to_string()));
}
