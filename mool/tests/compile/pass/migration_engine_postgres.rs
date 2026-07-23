use mool as db;
use db::migrations::engine::{
    DatabaseTrackingStore, DirectoryMigrationStore, EngineError, MigrationEngine,
    PostgresExecutor,
};
use db::sqlx::Connection as _;

#[derive(Debug, Clone, db::Model)]
#[table(name = "posts")]
struct Post {
    #[column(primary_key)]
    id: i64,
    title: String,
}

fn desired_schema() -> Result<db::schema::Schema, db::schema::SchemaLoadError> {
    db::schema().model::<Post>().build()
}

async fn migrate(database_url: &str) -> Result<(), EngineError> {
    let connection = db::sqlx::PgConnection::connect(database_url)
        .await
        .map_err(|error| EngineError::Config(error.to_string()))?;
    let schema = desired_schema().map_err(|error| EngineError::Config(error.to_string()))?;
    let mut engine = MigrationEngine::new(
        db::migrations::Dialect::Postgres,
        DirectoryMigrationStore::new("migrations"),
        DatabaseTrackingStore,
        PostgresExecutor::new(connection),
    );

    let _ = engine.make_named(schema, Some("create_posts"), &[]).await?;
    engine.apply(None, false).await?;
    Ok(())
}

fn main() {
    let _ = migrate;
}
