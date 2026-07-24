use mool as db;

#[derive(Debug, Clone, db::Model)]
#[schema(name = "legacy_rows")]
struct LegacyRow {
    id: i64,
}

fn main() {}
