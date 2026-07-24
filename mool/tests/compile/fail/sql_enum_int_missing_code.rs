use mool as db;

#[derive(Debug, Clone, Copy, PartialEq, Eq, db::SqlEnum)]
#[sql_enum(storage = "int", repr = "i16")]
enum Status {
    #[sql_enum(code = 1)]
    Draft,
    Published,
}

fn main() {}
