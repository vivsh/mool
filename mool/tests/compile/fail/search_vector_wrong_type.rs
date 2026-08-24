use mool as db;

#[derive(Debug, Clone, db::Model)]
struct Document {
    #[column(primary_key)]
    id: i64,
    #[column(vector(metric = "cosine"))]
    embedding: Vec<f32>,
}

fn main() {}
