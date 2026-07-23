use mool as db;
use mool::Model;

#[derive(Debug, Clone, db::Model)]
struct Post {
    id: i64,
}

fn main() {
    let posts = Post::table();
    let id = db::var::<i64>();
    let _ = db::from(&posts)
        .filter(posts.id.eq(&id))
        .bind(&id, "wrong".to_string());
}
