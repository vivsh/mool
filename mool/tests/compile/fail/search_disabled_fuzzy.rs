use mool as db;

#[derive(Debug, Clone, db::Model)]
struct Document {
    id: i64,
    #[column(fuzzy)]
    title: String,
}

fn main() {}
