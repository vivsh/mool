//! Synchronous typed-query terminal builders.
use std::marker::PhantomData;

use crate::interfaces::{Model, Record};

use super::ColumnSet;
use super::batch::{BatchPolicy, InsertConflict};
use super::executables::{
    All, BatchInsert, BatchUpdate, BatchUpsert, Count, Delete, Exists, First, Insert, One,
    ReturningBatchInsert, ReturningBatchUpdate, ReturningBatchUpsert, ReturningDelete,
    ReturningInsert, ReturningUpdate, Scalar, Slice, Update,
};
use super::expr::IntoExpr;
use super::scope::{QueryScope, ReturningScope};
use super::values::{NoRecord, WriteInput, WriteUsing, WriteValues};

impl QueryScope {
    /// Builds an executable that fetches all selected rows.
    pub fn all<T>(self) -> All<T>
    where
        T: Record,
    {
        All {
            scope: self,
            _marker: PhantomData,
        }
    }

    /// Builds an executable that fetches exactly one selected row.
    pub fn one<T>(self) -> One<T>
    where
        T: Record,
    {
        One {
            scope: self,
            _marker: PhantomData,
        }
    }

    /// Builds an executable that fetches the first selected row.
    pub fn first<T>(self) -> First<T>
    where
        T: Record,
    {
        First {
            scope: self,
            _marker: PhantomData,
        }
    }

    /// Builds an executable that fetches a limited selected row slice.
    pub fn slice<T>(self, offset: usize, count: usize) -> Slice<T>
    where
        T: Record,
    {
        Slice {
            scope: self,
            offset,
            count,
            _marker: PhantomData,
        }
    }

    /// Builds an executable that inserts one record.
    pub fn insert<W>(self, row: W) -> Insert<W>
    where
        W: WriteInput,
    {
        Insert { scope: self, row }
    }

    /// Builds an expression-only insert executable.
    #[doc(hidden)]
    pub fn insert_using<F>(self, f: F) -> Insert<WriteValues<'static, NoRecord>>
    where
        F: FnOnce(WriteUsing) -> WriteUsing,
    {
        Insert {
            scope: self,
            row: f(WriteUsing::new()).into_values(),
        }
    }

    /// Builds an executable that updates matching rows from one record.
    pub fn update<W>(self, row: W) -> Update<W>
    where
        W: WriteInput,
    {
        Update { scope: self, row }
    }

    /// Builds an expression-only update executable.
    #[doc(hidden)]
    pub fn update_using<F>(self, f: F) -> Update<WriteValues<'static, NoRecord>>
    where
        F: FnOnce(WriteUsing) -> WriteUsing,
    {
        Update {
            scope: self,
            row: f(WriteUsing::new()).into_values(),
        }
    }

    /// Builds an executable that deletes matching rows.
    pub fn delete(self) -> Delete {
        Delete { scope: self }
    }

    /// Builds a `COUNT(*)` executable.
    pub fn count(self) -> Count {
        Count { scope: self }
    }

    /// Builds an `EXISTS(...)` executable.
    pub fn exists(self) -> Exists {
        Exists { scope: self }
    }

    /// Builds a scalar-select executable.
    pub fn scalar<V>(self, expr: impl IntoExpr<V>) -> Scalar<V> {
        Scalar {
            scope: self,
            expr: expr.into_expr().node,
            _marker: PhantomData,
        }
    }

    /// Builds an executable that inserts multiple records.
    pub fn batch_insert<T>(self, rows: &[T]) -> BatchInsert<'_, T>
    where
        T: Record,
    {
        BatchInsert {
            scope: self,
            rows,
            policy: BatchPolicy::default(),
            conflict: InsertConflict::None,
        }
    }

    /// Builds an executable that upserts multiple records.
    pub fn batch_upsert<T, C>(self, rows: &[T], conflict: C) -> BatchUpsert<'_, T>
    where
        T: Record,
        C: ColumnSet,
    {
        let conflict = conflict.into_column_refs();
        BatchUpsert {
            scope: self,
            rows,
            conflict,
            update_columns: None,
            policy: BatchPolicy::default(),
        }
    }

    /// Builds an executable that updates multiple models by primary key.
    pub fn batch_update<T, C>(self, rows: &[T], columns: C) -> BatchUpdate<'_, T>
    where
        T: Model,
        C: ColumnSet,
    {
        BatchUpdate {
            scope: self,
            rows,
            update_columns: columns.into_column_refs(),
            policy: BatchPolicy::default(),
        }
    }
}

impl<R> ReturningScope<R>
where
    R: Record,
{
    /// Builds a returning insert executable.
    pub fn insert<W>(self, row: W) -> ReturningInsert<R, W>
    where
        W: WriteInput,
    {
        ReturningInsert {
            returning: self,
            row,
        }
    }

    /// Builds an expression-only returning insert executable.
    #[doc(hidden)]
    pub fn insert_using<F>(self, f: F) -> ReturningInsert<R, WriteValues<'static, NoRecord>>
    where
        F: FnOnce(WriteUsing) -> WriteUsing,
    {
        ReturningInsert {
            returning: self,
            row: f(WriteUsing::new()).into_values(),
        }
    }

    /// Builds a returning update executable.
    pub fn update<W>(self, row: W) -> ReturningUpdate<R, W>
    where
        W: WriteInput,
    {
        ReturningUpdate {
            returning: self,
            row,
        }
    }

    /// Builds an expression-only returning update executable.
    #[doc(hidden)]
    pub fn update_using<F>(self, f: F) -> ReturningUpdate<R, WriteValues<'static, NoRecord>>
    where
        F: FnOnce(WriteUsing) -> WriteUsing,
    {
        ReturningUpdate {
            returning: self,
            row: f(WriteUsing::new()).into_values(),
        }
    }

    /// Builds a returning delete executable.
    pub fn delete(self) -> ReturningDelete<R> {
        ReturningDelete { returning: self }
    }

    /// Builds a returning batch insert executable.
    pub fn batch_insert<T>(self, rows: &[T]) -> ReturningBatchInsert<'_, R, T>
    where
        T: Record,
    {
        ReturningBatchInsert {
            returning: self,
            rows,
            policy: BatchPolicy::default(),
            conflict: InsertConflict::None,
        }
    }

    /// Builds a returning batch upsert executable.
    pub fn batch_upsert<T, C>(self, rows: &[T], conflict: C) -> ReturningBatchUpsert<'_, R, T>
    where
        T: Record,
        C: ColumnSet,
    {
        let conflict = conflict.into_column_refs();
        ReturningBatchUpsert {
            returning: self,
            rows,
            conflict,
            update_columns: None,
            policy: BatchPolicy::default(),
        }
    }

    /// Builds a returning multi-row update executable.
    pub fn batch_update<T, C>(self, rows: &[T], columns: C) -> ReturningBatchUpdate<'_, R, T>
    where
        T: Model,
        C: ColumnSet,
    {
        ReturningBatchUpdate {
            returning: self,
            rows,
            update_columns: columns.into_column_refs(),
            policy: BatchPolicy::default(),
        }
    }
}
