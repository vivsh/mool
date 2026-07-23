#[cfg(feature = "migrations")]
static MIGRATIONS: mool::migrations::EmbeddedMigrations =
    mool::migrations::embedded_migrations!("tests/fixtures/migrations");

#[cfg(feature = "migrations")]
fn main() {
    let source = mool::migrations::root_migration(&MIGRATIONS);
    assert_eq!(source.embedded().files.len(), 2);
}

#[cfg(not(feature = "migrations"))]
fn main() {}
