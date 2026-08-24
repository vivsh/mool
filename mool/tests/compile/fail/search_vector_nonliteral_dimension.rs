use mool as db;

#[derive(Debug, Clone, db::Model)]
struct Document<const D: usize> {
    #[column(primary_key)]
    id: i64,
    #[column(vector(dimensions = D, metric = "cosine"))]
    embedding: db::types::Vector,
}

fn main() {}
