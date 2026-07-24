use sqlx::migrate::MigrateDatabase;

use crate::DbConf;

use super::{TestDatabaseError, target};

/// Creates a file-backed SQLite target that is isolated from the source URL.
pub(crate) async fn create(source: &DbConf) -> Result<target::TestTarget, TestDatabaseError> {
    let directory =
        tempfile::tempdir().map_err(|source| TestDatabaseError::TemporaryDirectory { source })?;
    let path = directory
        .path()
        .join(format!("{}.sqlite", target::generated_name()));
    let identity = path.display().to_string();
    let conf = target::sqlite_conf(source, &path)?;
    <sqlx::Sqlite as MigrateDatabase>::create_database(&conf.url)
        .await
        .map_err(|source| TestDatabaseError::Create {
            target: identity.clone(),
            source,
        })?;
    Ok(target::TestTarget::sqlite(conf, identity, directory))
}

/// Deletes the SQLite database file while its temporary directory remains owned.
pub(crate) async fn teardown(target: &target::TestTarget) -> Result<(), TestDatabaseError> {
    <sqlx::Sqlite as MigrateDatabase>::drop_database(&target.conf().url)
        .await
        .map_err(|source| TestDatabaseError::Teardown {
            target: target.identity().to_string(),
            source,
        })
}
