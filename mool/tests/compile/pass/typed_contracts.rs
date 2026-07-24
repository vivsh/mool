use mool as db;
use mool::Model;

#[derive(Debug, Clone, db::Model)]
#[table(name = "typed_posts")]
struct Post {
    #[column(primary_key)]
    id: i64,
    title: String,
}

#[derive(Debug, Clone, db::Record)]
#[table(name = "typed_posts")]
struct PostSummary {
    id: i64,
    title: String,
}

#[derive(Debug, Clone, db::Record)]
#[table(name = "typed_posts")]
struct PostPatch {
    title: String,
}

fn main() {
    let posts = Post::table();
    let id = db::var::<i64>().named("id");
    let output = db::out::<PostSummary>();

    let _ = db::from(&posts)
        .filter(posts.id.eq(&id))
        .filter(posts.id.in_values([1_i64, 2_i64]))
        .bind(&id, 1_i64)
        .all::<PostSummary>()
        .set(&output.id, posts.id.clone())
        .set(&output.title, posts.title.clone());

    let patch = PostPatch {
        title: "draft".to_string(),
    };
    let _ = db::from(&posts).update(&patch).set(
        &posts.title,
        db::val("updated".to_string()),
    );

    let subquery = db::from(&posts)
        .all::<PostSummary>()
        .subquery_as("typed_post_subquery");
    let _ = db::from(&subquery).all::<PostSummary>();

    let cte = db::from(&posts)
        .all::<PostSummary>()
        .cte_as("typed_post_cte");
    let _ = db::from(&cte).with(&cte).all::<PostSummary>();
}
