#![cfg(feature = "migrations")]

use mool as db;

static MIGRATIONS: db::migrations::EmbeddedMigrations =
    db::migrations::embed_migrations!("tests/fixtures/migrations");
static EMPTY_MIGRATIONS: db::migrations::EmbeddedMigrations =
    db::migrations::embed_migrations!("tests/fixtures/empty_migrations");

/// Verifies Mool's public macro embeds YAML migrations in sorted id order.
#[test]
fn embed_migrations_macro_embeds_sorted_yaml_files() {
    assert_eq!(MIGRATIONS.files.len(), 2);
    assert_eq!(MIGRATIONS.files[0].0, "0001_first");
    assert_eq!(MIGRATIONS.files[1].0, "0002_second");
    assert!(MIGRATIONS.files[0].1.contains("id: 0001_first"));
    assert!(MIGRATIONS.files[1].1.contains("id: 0002_second"));
}

/// Verifies Mool's macro records the source directory used for embedding.
#[test]
fn embed_migrations_macro_records_source_directory() {
    assert!(MIGRATIONS.dir.ends_with("tests/fixtures/migrations"));
}

/// Verifies an empty migration directory embeds as an empty source.
#[test]
fn embed_migrations_macro_allows_empty_sources() {
    assert!(EMPTY_MIGRATIONS.files.is_empty());
    assert!(EMPTY_MIGRATIONS.children.is_empty());
    assert!(
        EMPTY_MIGRATIONS
            .dir
            .ends_with("tests/fixtures/empty_migrations")
    );
}
