#![allow(dead_code)]

use mool as db;

#[derive(Debug, Clone, Copy, PartialEq, Eq, db::SqlEnum)]
enum Status {
    Draft,
    Published,
}

#[derive(Debug, Clone, db::Model)]
#[table(name = "posts")]
struct Post {
    id: i64,
    #[column(sql_enum, type = "text")]
    status: Status,
}

fn main() {}
