use mool as db;
use db::{FuzzyTextExt as _, Model as _};

#[derive(Debug, Clone, db::Model)]
struct Document {
    #[column(primary_key)]
    id: i64,
    #[column(fuzzy(index = "gist"))]
    title: String,
}

fn main() {
    let documents = Document::table();
    let _ = db::from(&documents)
        .filter(
            documents
                .title
                .fuzzy_match(db::val("mool".to_string()), db::val(0.4_f64)),
        )
        .sort(
            documents
                .title
                .fuzzy_distance(db::val("mool".to_string()))
                .asc(),
        )
        .all::<Document>();
}
