use mool as db;

#[derive(Debug, Clone, db::Model)]
#[table(full_text(name = "content", columns = ["missing"]))]
struct Document {
    #[column(primary_key)]
    id: i64,
    title: String,
}

fn main() {}
