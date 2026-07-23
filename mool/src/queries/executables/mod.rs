//! Executable typed-query terminals.
//!
//! Executables are the terminal, observable operations produced by query scopes.
//! They own planning and execution entrypoints, while scope composition and SQL
//! rendering stay in their own modules.

mod owned;
mod pagination;
mod read;
mod returning;
mod scalar;
mod types;
#[cfg(feature = "postgres")]
mod unnest;
mod write;

pub use types::{
    All, BatchInsert, BatchUpdate, BatchUpsert, Count, Delete, Exists, First, Insert, One,
    OwnedBatchInsert, OwnedBatchUpdate, OwnedBatchUpsert, OwnedInsert, OwnedUpdate,
    ReturningBatchInsert, ReturningBatchUpdate, ReturningBatchUpsert, ReturningDelete,
    ReturningInsert, ReturningUpdate, Scalar, Slice, Update,
};
#[cfg(feature = "postgres")]
pub use types::{
    OwnedPgUnnestBatchInsert, OwnedPgUnnestBatchUpsert, PgUnnestBatchInsert, PgUnnestBatchUpsert,
    ReturningPgUnnestBatchInsert, ReturningPgUnnestBatchUpsert,
};
