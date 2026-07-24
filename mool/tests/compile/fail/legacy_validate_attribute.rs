use mool as db;

#[derive(Debug, Clone, db::Record)]
#[table(name = "legacy_rows")]
#[validate(rule = "ignored")]
struct LegacyRow {
    id: i64,
}

fn main() {}
