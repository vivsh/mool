use mool as db;
use mool::Model;

#[derive(Debug, Clone, db::Model)]
#[table(name = "posts")]
struct Post {
    id: i64,
}

fn main() {
    let posts = Post::table();
    let _ = db::from(&posts).returning::<Post>();
}
