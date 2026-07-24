//! Migration command and engine types for Mool application integrations.
//!
//! Mool owns schema registration and embedded migration history. Gaman owns
//! the execution engine, so this module exposes its stable integration types
//! through Mool rather than requiring applications to couple to Gaman paths.

pub use gaman::core::{
    Answer, Clarification, ClarificationKind, ClarificationMessage, Decision, MigrationStore,
    OptionAction, PromptEngine, Severity, TrackingStore,
};
pub use gaman::runner_factory::{
    DirectoryMigrationStore, EmbeddedMigrationStore, LazyExecutor, NativeMigrationStore,
    NativeRunnerFactory,
};
pub use gaman::{
    ApplyCommand, COMMAND_PROTOCOL_VERSION, CommandDiagnostic, CommandEnvelope, CommandFailure,
    CommandRequest, CommandResponse, CommandResult, Config, DatabaseTrackingStore, DiagnosticCode,
    EngineError, Executor, ExecutorError, MakeCommand, MakeResult, MigrationEngine,
    MigrationRunner, RepairOptions, SchemaCheckFailure, SchemaCheckInput, SchemaCheckResult,
    SchemaCheckStatus, SchemaInspector, SqlInput, TlsMode,
};
pub use gaman::{RunnerCommand as MigrationCommand, RunnerCommandError as MigrationCommandError};

/// PostgreSQL SQLx executor used by the Mool migration engine.
#[cfg(feature = "postgres")]
pub use gaman::core::PostgresExecutor;

/// SQLite SQLx executor used by the Mool migration engine.
#[cfg(feature = "sqlite")]
pub use gaman::core::SqliteExecutor;

/// MySQL-family SQLx executor used by the Mool migration engine.
#[cfg(any(feature = "mysql", feature = "mariadb"))]
pub use gaman::core::MysqlFamilyExecutor;
