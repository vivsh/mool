use mool as db;

static MIGRATIONS: db::migrations::EmbeddedMigrations =
    db::migrations::embed_migrations!("../../../../mool/tests/fixtures/migrations");

fn main() {
    let _source = db::migrations::root_migration(&MIGRATIONS);
}
