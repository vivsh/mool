use mool as db;

#[derive(Debug, Clone, Copy, PartialEq, Eq, db::SqlEnum)]
enum Status {
    Draft,
    Published(i32),
}

fn main() {}
