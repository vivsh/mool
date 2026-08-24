use mool as db;

#[derive(Debug, Clone, db::Model)]
#[table(full_text(name = "title", columns = ["title"]))]
struct Document {
    #[column(primary_key)]
    id: i64,
    #[column(fuzzy(index = "none"))]
    title: String,
}

fn main() {}
