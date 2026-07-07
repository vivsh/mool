#![allow(dead_code)]

use mool as db;

static MIGRATIONS: db::EmbeddedMigrations =
    db::embedded_migrations!("tests/fixtures/migrations");

fn main() {
    let _source = db::root_migration(&MIGRATIONS);
}
