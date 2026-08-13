use mool as db;
use mool::Model;

#[derive(Debug, Clone, db::Model)]
#[table(name = "posts")]
struct Post {
    #[column(primary_key)]
    id: i64,
    title: String,
    created_at: i64,
}

#[derive(Debug, Clone, db::SortKey)]
#[sort(model = Post, max_terms = 2)]
enum PostSort {
    Title,
    #[sort(name = "newest", by = created_at)]
    Newest,
}

fn main() {
    let sort = db::Sort::<PostSort>::parse("-newest,title").expect("valid sort");
    let plan = db::from(&Post::table())
        .sort_with(&sort)
        .all::<Post>()
        .plan()
        .expect("valid plan");

    assert!(plan.sql.contains("ORDER BY posts.created_at DESC, posts.title ASC"));
}
