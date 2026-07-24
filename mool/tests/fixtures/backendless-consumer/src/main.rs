use mool as db;

static MIGRATIONS: db::migrations::EmbeddedMigrations =
    db::migrations::embedded_migrations!("migrations");

fn desired_schema() -> Result<db::schema::Schema, db::schema::SchemaLoadError> {
    db::schema::SchemaBuilder::new(db::migrations::Dialect::Sqlite).build()
}

fn main() {
    let mut registry = db::migrations::MigrationRegistry::new();
    registry
        .register(db::migrations::root_migration(&MIGRATIONS))
        .unwrap();
    registry
        .register_schema(db::migrations::root_schema(desired_schema))
        .unwrap();
    let _ = registry.schema_for(None);
}
