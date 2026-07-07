#[cfg(feature = "migrations")]
static MIGRATIONS: mool::EmbeddedMigrations =
    mool::embedded_migrations!("tests/fixtures/migrations");

#[cfg(feature = "migrations")]
fn main() {
    let source = mool::root_migration(&MIGRATIONS);
    assert_eq!(source.embedded().files.len(), 2);
}

#[cfg(not(feature = "migrations"))]
fn main() {}
