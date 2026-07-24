use mool as db;

#[derive(Debug, Clone, db::Model)]
#[table(name = "malformed_references")]
struct MalformedReference {
    id: i64,
    #[column(reference = "missing_separator")]
    parent_id: i64,
}

fn main() {}
