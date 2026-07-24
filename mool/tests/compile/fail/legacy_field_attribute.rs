use mool as db;

#[derive(Debug, Clone, db::Record)]
#[table(name = "legacy_rows")]
struct LegacyRow {
    #[field(skip)]
    id: i64,
}

fn main() {}
