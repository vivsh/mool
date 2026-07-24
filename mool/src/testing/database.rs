use tokio::runtime::Handle;

use crate::{DbConf, DbPool};

use super::{TestDatabaseError, target};

/// Owns an isolated database and its application-facing Mool pool.
pub struct TestDatabase {
    pool: DbPool,
    conf: DbConf,
    target: Option<target::TestTarget>,
    runtime: Option<Handle>,
    preserve: bool,
}

impl TestDatabase {
    /// Stores an already-created target and captures its current Tokio runtime for drop cleanup.
    pub(crate) fn new(pool: DbPool, target: target::TestTarget, preserve: bool) -> Self {
        Self {
            conf: target.conf().clone(),
            pool,
            target: Some(target),
            runtime: Handle::try_current().ok(),
            preserve,
        }
    }

    /// Returns the normal Mool pool connected to this isolated target.
    pub fn pool(&self) -> &DbPool {
        &self.pool
    }

    /// Returns the mutable pool handle required by Mool query execution.
    pub fn pool_mut(&mut self) -> &mut DbPool {
        &mut self.pool
    }

    /// Returns the generated configuration for this isolated target.
    pub fn conf(&self) -> &DbConf {
        &self.conf
    }

    /// Closes the pool and deterministically deletes the isolated database.
    pub async fn teardown(mut self) -> Result<(), TestDatabaseError> {
        let Some(target) = self.target.take() else {
            return Ok(());
        };
        if let Err(error) = cleanup(&self.pool, &target).await {
            self.target = Some(target);
            return Err(error);
        }
        Ok(())
    }
}

impl Drop for TestDatabase {
    fn drop(&mut self) {
        if self.preserve {
            return;
        }
        let Some(target) = self.target.take() else {
            return;
        };
        let Some(runtime) = self.runtime.take() else {
            report_cleanup_runtime_unavailable(target.identity());
            return;
        };
        let pool = self.pool.clone();
        runtime.spawn(async move {
            if let Err(error) = cleanup(&pool, &target).await {
                eprintln!("Mool test database cleanup failed: {error}");
            }
        });
    }
}

/// Closes the shared pool before removing the isolated database target.
async fn cleanup(pool: &DbPool, target: &target::TestTarget) -> Result<(), TestDatabaseError> {
    pool.close().await;
    target::teardown(target).await
}

/// Reports a target that cannot be cleaned automatically outside a Tokio runtime.
fn report_cleanup_runtime_unavailable(target: &str) {
    eprintln!("Mool test database cleanup was skipped for {target}: no Tokio runtime");
}
