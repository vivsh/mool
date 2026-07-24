use sqlx::migrate::MigrateDatabase;

use crate::DbConf;

use super::{TestDatabaseError, target};

/// Creates an isolated MySQL-family database through SQLx's maintenance API.
pub(crate) async fn create(source: &DbConf) -> Result<target::TestTarget, TestDatabaseError> {
    let name = target::generated_name();
    let conf = target::server_conf(source, &name)?;
    <sqlx::MySql as MigrateDatabase>::create_database(&conf.url)
        .await
        .map_err(|source| TestDatabaseError::Create {
            target: name.clone(),
            source,
        })?;
    Ok(target::TestTarget::server(conf, name))
}

/// Deletes the isolated MySQL-family database after its pool is closed.
pub(crate) async fn teardown(target: &target::TestTarget) -> Result<(), TestDatabaseError> {
    <sqlx::MySql as MigrateDatabase>::drop_database(&target.conf().url)
        .await
        .map_err(|source| TestDatabaseError::Teardown {
            target: target.identity().to_string(),
            source,
        })
}
