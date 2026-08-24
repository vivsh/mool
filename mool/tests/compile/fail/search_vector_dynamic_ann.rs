use mool as db;

#[derive(Debug, Clone, db::Model)]
struct Document {
    #[column(primary_key)]
    id: i64,
    #[column(vector(index = "hnsw"))]
    embedding: db::types::Vector,
}

fn main() {}
