use mool as db;

#[derive(Debug, Clone, db::Model)]
#[table(name = "posts")]
struct Post {
    id: i64,
    title: String,
}

#[derive(Debug, Clone, db::Filterable)]
#[filter(model = Post)]
struct BadFilter {
    #[filter(op = "contains", column = "title")]
    title: Option<String>,
}

fn main() {}
