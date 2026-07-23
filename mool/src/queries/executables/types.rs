//! Executable type definitions.

use std::marker::PhantomData;

use super::super::batch::{BatchPolicy, InsertConflict};
use super::super::expr::{ColumnRef, ExprNode};
use super::super::scope::{QueryScope, ReturningScope};

/// Fetch-all executable produced by `all::<T>()`.
#[derive(Clone)]
pub struct All<T> {
    pub(in crate::queries) scope: QueryScope,
    pub(in crate::queries) _marker: PhantomData<fn() -> T>,
}

/// Fetch-one executable produced by `one::<T>()`.
#[derive(Clone)]
pub struct One<T> {
    pub(in crate::queries) scope: QueryScope,
    pub(in crate::queries) _marker: PhantomData<fn() -> T>,
}

/// Fetch-optional executable produced by `first::<T>()`.
#[derive(Clone)]
pub struct First<T> {
    pub(in crate::queries) scope: QueryScope,
    pub(in crate::queries) _marker: PhantomData<fn() -> T>,
}

/// Limited fetch executable produced by `slice::<T>(...)`.
#[derive(Clone)]
pub struct Slice<T> {
    pub(in crate::queries) scope: QueryScope,
    pub(in crate::queries) offset: usize,
    pub(in crate::queries) count: usize,
    pub(in crate::queries) _marker: PhantomData<fn() -> T>,
}

/// `COUNT(*)` executable.
pub struct Count {
    pub(in crate::queries) scope: QueryScope,
}

/// `EXISTS(...)` executable.
pub struct Exists {
    pub(in crate::queries) scope: QueryScope,
}

/// Scalar select executable.
pub struct Scalar<V> {
    pub(in crate::queries) scope: QueryScope,
    pub(in crate::queries) expr: ExprNode,
    pub(in crate::queries) _marker: PhantomData<fn() -> V>,
}

/// Single-row insert executable.
pub struct Insert<T> {
    pub(in crate::queries) scope: QueryScope,
    pub(in crate::queries) row: T,
}

/// Single-row update executable.
pub struct Update<T> {
    pub(in crate::queries) scope: QueryScope,
    pub(in crate::queries) row: T,
}

/// Delete executable.
pub struct Delete {
    pub(in crate::queries) scope: QueryScope,
}

/// Multi-row insert executable.
pub struct BatchInsert<'a, T> {
    pub(in crate::queries) scope: QueryScope,
    pub(in crate::queries) rows: &'a [T],
    pub(in crate::queries) policy: BatchPolicy,
    pub(crate) conflict: InsertConflict,
}

/// Multi-row upsert executable.
pub struct BatchUpsert<'a, T> {
    pub(in crate::queries) scope: QueryScope,
    pub(in crate::queries) rows: &'a [T],
    pub(in crate::queries) conflict: Vec<ColumnRef>,
    pub(in crate::queries) update_columns: Option<Vec<ColumnRef>>,
    pub(in crate::queries) policy: BatchPolicy,
}

/// Multi-row update executable keyed by each model's primary key.
pub struct BatchUpdate<'a, T> {
    pub(in crate::queries) scope: QueryScope,
    pub(in crate::queries) rows: &'a [T],
    pub(in crate::queries) update_columns: Vec<ColumnRef>,
    pub(in crate::queries) policy: BatchPolicy,
}

/// Owned single-row insert executable.
pub struct OwnedInsert<T> {
    pub(in crate::queries) scope: QueryScope,
    pub(in crate::queries) row: T,
}

/// Owned single-row update executable.
pub struct OwnedUpdate<T> {
    pub(in crate::queries) scope: QueryScope,
    pub(in crate::queries) row: T,
}

/// Owned multi-row insert executable.
pub struct OwnedBatchInsert<T> {
    pub(in crate::queries) scope: QueryScope,
    pub(in crate::queries) rows: Vec<T>,
    pub(in crate::queries) policy: BatchPolicy,
    pub(crate) conflict: InsertConflict,
}

/// Owned multi-row upsert executable.
pub struct OwnedBatchUpsert<T> {
    pub(in crate::queries) scope: QueryScope,
    pub(in crate::queries) rows: Vec<T>,
    pub(in crate::queries) conflict: Vec<ColumnRef>,
    pub(in crate::queries) update_columns: Option<Vec<ColumnRef>>,
    pub(in crate::queries) policy: BatchPolicy,
}

/// Owned multi-row update executable.
pub struct OwnedBatchUpdate<T> {
    pub(in crate::queries) scope: QueryScope,
    pub(in crate::queries) rows: Vec<T>,
    pub(in crate::queries) update_columns: Vec<ColumnRef>,
    pub(in crate::queries) policy: BatchPolicy,
}

/// Returning insert executable.
pub struct ReturningInsert<R, T> {
    pub(in crate::queries) returning: ReturningScope<R>,
    pub(in crate::queries) row: T,
}

/// Returning update executable.
pub struct ReturningUpdate<R, T> {
    pub(in crate::queries) returning: ReturningScope<R>,
    pub(in crate::queries) row: T,
}

/// Returning delete executable.
pub struct ReturningDelete<R> {
    pub(in crate::queries) returning: ReturningScope<R>,
}

/// Returning multi-row insert executable.
pub struct ReturningBatchInsert<'a, R, T> {
    pub(in crate::queries) returning: ReturningScope<R>,
    pub(in crate::queries) rows: &'a [T],
    pub(in crate::queries) policy: BatchPolicy,
    pub(crate) conflict: InsertConflict,
}

/// Returning multi-row upsert executable.
pub struct ReturningBatchUpsert<'a, R, T> {
    pub(in crate::queries) returning: ReturningScope<R>,
    pub(in crate::queries) rows: &'a [T],
    pub(in crate::queries) conflict: Vec<ColumnRef>,
    pub(in crate::queries) update_columns: Option<Vec<ColumnRef>>,
    pub(in crate::queries) policy: BatchPolicy,
}

/// Returning multi-row update executable.
pub struct ReturningBatchUpdate<'a, R, T> {
    pub(in crate::queries) returning: ReturningScope<R>,
    pub(in crate::queries) rows: &'a [T],
    pub(in crate::queries) update_columns: Vec<ColumnRef>,
    pub(in crate::queries) policy: BatchPolicy,
}

/// PostgreSQL columnar batch insert using `UNNEST`.
#[cfg(feature = "postgres")]
pub struct PgUnnestBatchInsert<'a, T> {
    pub(crate) inner: BatchInsert<'a, T>,
}

/// PostgreSQL columnar batch upsert using `UNNEST`.
#[cfg(feature = "postgres")]
pub struct PgUnnestBatchUpsert<'a, T> {
    pub(crate) inner: BatchUpsert<'a, T>,
}

/// Owned PostgreSQL columnar batch insert using `UNNEST`.
#[cfg(feature = "postgres")]
pub struct OwnedPgUnnestBatchInsert<T> {
    pub(crate) inner: OwnedBatchInsert<T>,
}

/// Owned PostgreSQL columnar batch upsert using `UNNEST`.
#[cfg(feature = "postgres")]
pub struct OwnedPgUnnestBatchUpsert<T> {
    pub(crate) inner: OwnedBatchUpsert<T>,
}

/// PostgreSQL returning columnar batch insert using `UNNEST`.
#[cfg(feature = "postgres")]
pub struct ReturningPgUnnestBatchInsert<'a, R, T> {
    pub(crate) inner: ReturningBatchInsert<'a, R, T>,
}

/// PostgreSQL returning columnar batch upsert using `UNNEST`.
#[cfg(feature = "postgres")]
pub struct ReturningPgUnnestBatchUpsert<'a, R, T> {
    pub(crate) inner: ReturningBatchUpsert<'a, R, T>,
}
