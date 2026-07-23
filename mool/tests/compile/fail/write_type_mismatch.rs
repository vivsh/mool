use mool as db;
use mool::Model;

#[derive(Debug, Clone, db::Model)]
struct Post {
    id: i64,
}

fn main() {
    let posts = Post::table();
    let _ = db::from(&posts)
        .update_using(|write| write.set(&posts.id, db::val("wrong".to_string())));
}
