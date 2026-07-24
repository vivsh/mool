use crate::{DbConf, DbPool};

use super::{TestDatabase, TestDatabaseError, target};
#[cfg(all(feature = "migrations", any(feature = "postgres", feature = "sqlite")))]
use crate::migrations::MigrationRegistry;

/// Starts configuration of an isolated test database.
pub fn setup(conf: DbConf) -> TestDatabaseBuilder {
    TestDatabaseBuilder {
        conf,
        preserve: false,
        #[cfg(all(feature = "migrations", any(feature = "postgres", feature = "sqlite")))]
        migrations: None,
    }
}

/// Configures one isolated test database before it is created.
pub struct TestDatabaseBuilder {
    conf: DbConf,
    preserve: bool,
    #[cfg(all(feature = "migrations", any(feature = "postgres", feature = "sqlite")))]
    migrations: Option<MigrationRegistry>,
}

impl TestDatabaseBuilder {
    /// Disables automatic cleanup on drop while retaining explicit [`TestDatabase::teardown`].
    pub fn preserve(mut self) -> Self {
        self.preserve = true;
        self
    }

    /// Applies a snapshot of the application's registered migrations during setup.
    #[cfg(all(feature = "migrations", any(feature = "postgres", feature = "sqlite")))]
    pub fn with_migrations(mut self, registry: &MigrationRegistry) -> Self {
        self.migrations = Some(registry.clone());
        self
    }

    /// Creates the isolated target, optionally applies migrations, and returns its pool handle.
    pub async fn create(self) -> Result<TestDatabase, TestDatabaseError> {
        let target = target::create(&self.conf).await?;
        if let Err(error) = self.apply_migrations(&target).await {
            cleanup_failed_setup(&target).await;
            return Err(error);
        }
        let pool = match self.open_pool(&target).await {
            Ok(pool) => pool,
            Err(error) => {
                cleanup_failed_setup(&target).await;
                return Err(error);
            }
        };
        Ok(TestDatabase::new(pool, target, self.preserve))
    }

    /// Applies the configured migration snapshot without opening the returned Mool pool.
    async fn apply_migrations(&self, target: &target::TestTarget) -> Result<(), TestDatabaseError> {
        #[cfg(all(feature = "migrations", any(feature = "postgres", feature = "sqlite")))]
        if let Some(registry) = &self.migrations {
            return super::migrations::apply(target, registry.clone()).await;
        }
        let _ = target;
        Ok(())
    }

    /// Opens the caller-visible pool only after target provisioning succeeds.
    async fn open_pool(&self, target: &target::TestTarget) -> Result<DbPool, TestDatabaseError> {
        DbPool::from_conf(target.conf())
            .await
            .map_err(|source| TestDatabaseError::Pool { source })
    }
}

/// Removes a target that failed before ownership could transfer to [`TestDatabase`].
async fn cleanup_failed_setup(target: &target::TestTarget) {
    if let Err(error) = target::teardown(target).await {
        eprintln!("Mool test database setup cleanup failed: {error}");
    }
}
