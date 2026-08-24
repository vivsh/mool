use mool as db;

#[derive(Debug, Clone, db::Model)]
struct Document {
    #[column(primary_key)]
    id: i64,
    #[column(vector(dimensions = 0, metric = "cosine"))]
    embedding: db::types::Vector,
}

fn main() {}
