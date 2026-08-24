use mool as db;
use db::{Model as _, VectorSimilarityExt as _};

#[derive(Debug, Clone, db::Model)]
struct Document {
    #[column(primary_key)]
    id: i64,
    embedding: db::types::Vector,
}

fn main() {
    let documents = Document::table();
    let needle = db::var::<db::types::Vector>().named("needle");
    let _ = db::from(&documents)
        .sort(documents.embedding.cosine_distance(&needle).asc())
        .bind(&needle, db::types::Vector::from_array([1.0, 2.0, 3.0]))
        .slice::<Document>(0, 10);
}
