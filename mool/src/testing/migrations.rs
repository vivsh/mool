use std::sync::Arc;

use crate::migrations::{MigrationRegistry, engine};

use super::{TestDatabaseError, target};

/// Applies one registry snapshot to an isolated PostgreSQL or SQLite target.
pub(crate) async fn apply(
    target: &target::TestTarget,
    registry: MigrationRegistry,
) -> Result<(), TestDatabaseError> {
    let root = registry
        .root()
        .ok_or(TestDatabaseError::MissingMigrationRoot)?;
    let config = engine::Config::new(
        target.conf().url.clone(),
        root.dir(),
        root.dir().join("schema.yaml"),
        selected_dialect(),
    );
    let mut runner = engine::NativeRunnerFactory::from_store(config, Arc::new(registry)).build();
    runner
        .run_command(&engine::MigrationCommand::Apply(
            engine::ApplyCommand::Execute {
                target: None,
                fake: false,
                fake_verified: false,
                schemas: Vec::new(),
            },
        ))
        .await
        .map(|_| ())
        .map_err(|source| TestDatabaseError::Migrations {
            target: target.identity().to_string(),
            source,
        })
}

/// Maps the selected Mool backend feature to Gaman's migration dialect.
fn selected_dialect() -> crate::migrations::Dialect {
    #[cfg(feature = "postgres")]
    {
        crate::migrations::Dialect::Postgres
    }
    #[cfg(feature = "sqlite")]
    {
        crate::migrations::Dialect::Sqlite
    }
}
