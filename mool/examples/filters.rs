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

#[derive(Debug, Clone, Copy)]
enum PostOrdering {
    Title,
}

impl db::Sortable for PostOrdering {
    type Model = Post;

    fn apply_sort(&self, sort: db::SortBuilder<Self::Model>) -> db::SortBuilder<Self::Model> {
        let order = match self {
            Self::Title => sort.title.asc(),
        };
        sort.sort(order)
    }
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
        .sort_with(&PostOrdering::Title)
        .all::<Post>()
        .plan()?;

    assert!(plan.sql.contains("ILIKE"));
    assert!(plan.sql.contains("ORDER BY posts.title ASC"));
    assert_eq!(plan.total_bind_count, 4);
    Ok(())
}
