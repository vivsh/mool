use crate::DbConf;

use super::TestDatabaseError;
#[cfg(any(feature = "mysql", feature = "mariadb"))]
use super::mysql as selected_backend;
#[cfg(feature = "postgres")]
use super::postgres as selected_backend;
#[cfg(feature = "sqlite")]
use super::sqlite as selected_backend;

/// One isolated database target owned by a [`super::TestDatabase`].
pub(crate) struct TestTarget {
    conf: DbConf,
    identity: String,
    #[cfg(feature = "sqlite")]
    _temp_dir: Option<tempfile::TempDir>,
}

impl TestTarget {
    /// Creates a server-backed target from a derived configuration.
    #[cfg(any(feature = "postgres", feature = "mysql", feature = "mariadb"))]
    pub(crate) fn server(conf: DbConf, identity: String) -> Self {
        Self {
            conf,
            identity,
            #[cfg(feature = "sqlite")]
            _temp_dir: None,
        }
    }

    /// Creates a SQLite target and retains its temporary directory until cleanup.
    #[cfg(feature = "sqlite")]
    pub(crate) fn sqlite(conf: DbConf, identity: String, temp_dir: tempfile::TempDir) -> Self {
        Self {
            conf,
            identity,
            _temp_dir: Some(temp_dir),
        }
    }

    /// Returns the configuration for the isolated database target.
    pub(crate) fn conf(&self) -> &DbConf {
        &self.conf
    }

    /// Returns a safe identifier for diagnostics and cleanup logs.
    pub(crate) fn identity(&self) -> &str {
        &self.identity
    }
}

/// Provisions one isolated target for the selected backend.
pub(crate) async fn create(conf: &DbConf) -> Result<TestTarget, TestDatabaseError> {
    selected_backend::create(conf).await
}

/// Removes one isolated target after all owned pool connections are closed.
pub(crate) async fn teardown(target: &TestTarget) -> Result<(), TestDatabaseError> {
    selected_backend::teardown(target).await
}

/// Creates a collision-resistant database name accepted by every server backend.
pub(crate) fn generated_name() -> String {
    format!("mool_test_{}", uuid::Uuid::new_v4().simple())
}

/// Derives a server target URL without changing pool settings or transport options.
#[cfg(any(feature = "postgres", feature = "mysql", feature = "mariadb"))]
pub(crate) fn server_conf(source: &DbConf, name: &str) -> Result<DbConf, TestDatabaseError> {
    let mut url = url::Url::parse(&source.url).map_err(|error| TestDatabaseError::TargetUrl {
        reason: error.to_string(),
    })?;
    url.set_path(&format!("/{name}"));

    let mut target = source.clone();
    target.url = url.into();
    Ok(target)
}

/// Converts a source SQLite URL into an isolated temporary-file configuration.
#[cfg(feature = "sqlite")]
pub(crate) fn sqlite_conf(
    source: &DbConf,
    path: &std::path::Path,
) -> Result<DbConf, TestDatabaseError> {
    let source_url =
        url::Url::parse(&source.url).map_err(|error| TestDatabaseError::TargetUrl {
            reason: error.to_string(),
        })?;
    let file_url = url::Url::from_file_path(path).map_err(|_| TestDatabaseError::TargetUrl {
        reason: "temporary SQLite path cannot be represented as a URL".to_string(),
    })?;
    let encoded_path =
        file_url
            .as_str()
            .strip_prefix("file:")
            .ok_or_else(|| TestDatabaseError::TargetUrl {
                reason: "temporary SQLite file URL is missing its scheme".to_string(),
            })?;
    let mut target_url = url::Url::parse(&format!("sqlite:{encoded_path}")).map_err(|error| {
        TestDatabaseError::TargetUrl {
            reason: error.to_string(),
        }
    })?;
    for (key, value) in source_url.query_pairs().filter(|(key, _)| key != "mode") {
        target_url.query_pairs_mut().append_pair(&key, &value);
    }

    let mut target = source.clone();
    target.url = target_url.into();
    Ok(target)
}
