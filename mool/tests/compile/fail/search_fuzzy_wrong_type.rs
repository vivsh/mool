use mool as db;

#[derive(Debug, Clone, db::Model)]
struct Document {
    #[column(primary_key)]
    id: i64,
    #[column(fuzzy(index = "gin"))]
    title: Option<String>,
}

fn main() {}
