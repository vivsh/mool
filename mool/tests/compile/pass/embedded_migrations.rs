#![allow(dead_code)]

use mool as db;

static MIGRATIONS: db::migrations::EmbeddedMigrations =
    db::migrations::embedded_migrations!("tests/fixtures/migrations");

fn main() {
    let _source = db::migrations::root_migration(&MIGRATIONS);
}
