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

#[derive(Debug, Clone, db::SortKey)]
#[sort(model = Post)]
enum PostSort {
    Title,
}

fn main() -> Result<(), db::QueryError> {
    let posts = Post::table();
    let filter = PostFilter {
        published: Some(true),
        q: Some("%mool%".to_string()),
        ids: vec![1, 2],
    };
    let sort = db::Sort::<PostSort>::parse("-title")
        .map_err(|error| db::QueryError::BindError(error.to_string()))?;
    let plan = db::from(&posts)
        .filter_with(&filter)
        .sort_with(&sort)
        .all::<Post>()
        .plan()?;

    assert!(plan.sql.contains("ILIKE"));
    assert!(plan.sql.contains("ORDER BY posts.title ASC"));
    assert_eq!(plan.total_bind_count, 4);
    Ok(())
}
