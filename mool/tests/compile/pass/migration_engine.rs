use mool as db;
use db::migrations::engine::{
    ApplyCommand, Executor, MigrationCommand, MigrationEngine, MigrationRunner, MigrationStore,
    SchemaInspector, TrackingStore,
};

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
    let _command = MigrationCommand::Apply(ApplyCommand::Plan);
}
