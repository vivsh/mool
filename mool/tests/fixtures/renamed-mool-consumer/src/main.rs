static MIGRATIONS: db::migrations::EmbeddedMigrations =
    db::migrations::embed_migrations!("../migrations");

fn main() {
    let _source = db::migrations::root_migration(&MIGRATIONS);
}
