use mool as db;
use db::migrations::engine::{
    DatabaseTrackingStore, DirectoryMigrationStore, MigrationEngine, SqliteExecutor,
};

fn accepts_sqlite_engine(
    _engine: MigrationEngine<DirectoryMigrationStore, DatabaseTrackingStore, SqliteExecutor>,
) {
}

fn main() {}
