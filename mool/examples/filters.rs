use mool as db;
use mool::Model;

#[derive(Debug, Clone, db::Model)]
#[table(name = "posts")]
struct Post {
    id: i64,
    title: String,
    published: bool,
}

#[derive(Debug, Clone, db::Filterable)]
#[filter(model = Post)]
struct PostFilter {
    #[filter(op = "eq")]
    published: Option<bool>,
    #[filter(op = "ilike", column = "title")]
    q: Option<String>,
    #[filter(op = "in", column = "id")]
    ids: Vec<i64>,
}

fn main() -> Result<(), db::QueryError> {
    let posts = Post::table();
    let filter = PostFilter {
        published: Some(true),
        q: Some("%mool%".to_string()),
        ids: vec![1, 2],
    };
    let plan = db::from(&posts)
        .filter_with(&filter)
        .all::<Post>()
        .plan(db::queries::Dialect::Postgres)?;

    assert!(plan.sql.contains("ILIKE"));
    assert_eq!(plan.total_bind_count, 4);
    Ok(())
}
