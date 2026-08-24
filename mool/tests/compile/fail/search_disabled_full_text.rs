use mool as db;

#[derive(Debug, Clone, db::Model)]
#[table(full_text(name = "content", columns = ["title"]))]
struct Document {
    id: i64,
    title: String,
}

fn main() {}
