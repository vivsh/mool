use mool as db;

static MIGRATIONS: db::migrations::EmbeddedMigrations =
    db::migrations::embedded_migrations!("migrations");

fn accepts_backendless_facade() {
    let _arguments = db::backend::Arguments::default();
    let _pool = db::DbPool::from_pool(db::backend::Pool);
    let _config = db::DbConf::default();
    let _storage = db::schema::GeneratedStorage::Stored;
}

async fn opens_a_disabled_pool() {
    let pool = db::DbPool::from_conf(&db::DbConf::default())
        .await
        .unwrap();
    assert!(!pool.is_configured());
}

fn desired_schema() -> Result<db::schema::Schema, db::schema::SchemaLoadError> {
    db::schema::SchemaBuilder::new(db::migrations::Dialect::Sqlite).build()
}

fn main() {
    accepts_backendless_facade();
    let _ = opens_a_disabled_pool;
    let mut registry = db::migrations::MigrationRegistry::new();
    registry
        .register(db::migrations::root_migration(&MIGRATIONS))
        .unwrap();
    registry
        .register_schema(db::migrations::root_schema(desired_schema))
        .unwrap();
    let _ = registry.schema_for(None);
}
