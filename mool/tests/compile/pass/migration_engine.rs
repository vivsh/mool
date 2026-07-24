use db::migrations::engine::{
    ApplyCommand, COMMAND_PROTOCOL_VERSION, CommandDiagnostic, CommandEnvelope, CommandFailure,
    CommandRequest, CommandResponse, CommandResult, Config, DiagnosticCode, Executor, MakeCommand,
    MigrationCommand, MigrationCommandError, MigrationEngine, MigrationRunner, MigrationStore,
    NativeRunnerFactory, RepairOptions, SchemaCheckFailure, SchemaCheckInput, SchemaCheckResult,
    SchemaCheckStatus, SchemaInspector, SqlInput, TrackingStore,
};
use mool as db;

fn accepts_engine<M, T, E>(_engine: MigrationEngine<M, T, E>)
where
    M: MigrationStore,
    T: TrackingStore,
    E: Executor,
{
}

fn accepts_runner<M, T, E>(_runner: MigrationRunner<M, T, E>)
where
    M: MigrationStore,
    T: TrackingStore,
    E: Executor + SchemaInspector,
{
}

fn main() {
    let command = MigrationCommand::Apply(ApplyCommand::Plan);
    let _: CommandRequest = command.clone();
    let _envelope = CommandEnvelope {
        protocol_version: COMMAND_PROTOCOL_VERSION,
        command: command.clone(),
    };
    let _response = CommandResponse::new(CommandResult::Pending(Vec::new()));
    let _make = MakeCommand::Empty {
        name: "baseline".to_string(),
    };
    let _schema_input = SchemaCheckInput::Sql(SqlInput {
        name: "schema.sql".to_string(),
        sql: "SELECT 1".to_string(),
    });
    let _schema_result = SchemaCheckResult {
        name: "schema.sql".to_string(),
        status: SchemaCheckStatus::Ignored {
            reason: "fixture".to_string(),
        },
    };
    let _schema_failure = SchemaCheckFailure::Segmentation {
        line: None,
        column: None,
        message: "fixture".to_string(),
    };
    let _repair = RepairOptions::default();
    let _: Option<CommandDiagnostic> = None;
    let _: Option<CommandFailure> = None;
    let _: Option<MigrationCommandError> = None;
    let _: Option<DiagnosticCode> = None;

    let config = Config::new(
        "sqlite::memory:".to_string(),
        "migrations".into(),
        "schema.yaml".into(),
        db::migrations::Dialect::Sqlite,
    );
    let mut runner = NativeRunnerFactory::from_directory(config).build();
    assert_send(runner.run_command(&command));
}

fn assert_send<T: Send>(_future: T) {}
