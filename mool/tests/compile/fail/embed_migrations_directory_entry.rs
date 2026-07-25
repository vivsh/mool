use mool as db;

static MIGRATIONS: db::migrations::EmbeddedMigrations =
    db::migrations::embed_migrations!("../../../../mool/tests/fixtures/invalid_migrations");

fn main() {
    let _ = MIGRATIONS;
}
