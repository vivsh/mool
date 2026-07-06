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
mod write;

pub use types::{
    All, BatchInsert, BatchUpsert, Count, Delete, Exists, First, Insert, One, OwnedBatchInsert,
    OwnedBatchUpsert, OwnedInsert, OwnedUpdate, ReturningBatchInsert, ReturningBatchUpsert,
    ReturningDelete, ReturningInsert, ReturningUpdate, Scalar, Slice, Update,
};
