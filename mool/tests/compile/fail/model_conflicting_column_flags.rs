use mool as db;

#[derive(Debug, Clone, db::Model)]
#[table(name = "conflicting_columns")]
struct ConflictingColumns {
    #[column(primary_key, skip)]
    id: i64,
}

fn main() {}
