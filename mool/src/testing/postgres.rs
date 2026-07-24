use sqlx::migrate::MigrateDatabase;

use crate::DbConf;

use super::{TestDatabaseError, target};

/// Creates an isolated PostgreSQL database through SQLx's maintenance API.
pub(crate) async fn create(source: &DbConf) -> Result<target::TestTarget, TestDatabaseError> {
    let name = target::generated_name();
    let conf = target::server_conf(source, &name)?;
    <sqlx::Postgres as MigrateDatabase>::create_database(&conf.url)
        .await
        .map_err(|source| TestDatabaseError::Create {
            target: name.clone(),
            source,
        })?;
    Ok(target::TestTarget::server(conf, name))
}

/// Force-removes PostgreSQL connections before deleting the isolated database.
pub(crate) async fn teardown(target: &target::TestTarget) -> Result<(), TestDatabaseError> {
    <sqlx::Postgres as MigrateDatabase>::force_drop_database(&target.conf().url)
        .await
        .map_err(|source| TestDatabaseError::Teardown {
            target: target.identity().to_string(),
            source,
        })
}
