//! Owned write executable implementations.

use crate::executor::{DbError, DbSession};
use crate::interfaces::{Model, Record};
use crate::placeholders::Dialect;

use super::super::plan::QueryPlan;
use super::super::{BatchPlan, ColumnSet};
use super::{
    BatchInsert, BatchUpdate, BatchUpsert, Insert, OwnedBatchInsert, OwnedBatchUpdate,
    OwnedBatchUpsert, OwnedInsert, OwnedUpdate, Update,
};
use crate::QueryError;

impl<T> OwnedInsert<T>
where
    T: Record,
{
    /// Renders SQL and parameter metadata without executing the insert.
    pub fn plan(&self) -> Result<QueryPlan, QueryError> {
        self.scope.plan_insert(&&self.row, Dialect::active())
    }

    /// Executes this owned insert.
    pub async fn exec<S>(self, session: &mut S) -> Result<u64, DbError>
    where
        S: DbSession,
    {
        Insert {
            scope: self.scope,
            row: &self.row,
        }
        .exec(session)
        .await
    }
}

impl<T> OwnedUpdate<T>
where
    T: Record,
{
    /// Renders SQL and parameter metadata without executing the update.
    pub fn plan(&self) -> Result<QueryPlan, QueryError> {
        self.scope.plan_update(&&self.row, Dialect::active())
    }

    /// Executes this owned update.
    pub async fn exec<S>(self, session: &mut S) -> Result<u64, DbError>
    where
        S: DbSession,
    {
        Update {
            scope: self.scope,
            row: &self.row,
        }
        .exec(session)
        .await
    }
}

impl<T> OwnedBatchInsert<T>
where
    T: Record + 'static,
{
    /// Limits the maximum rows rendered into each SQL statement.
    pub fn batch_size(mut self, size: usize) -> Self {
        self.policy = self.policy.with_size(size);
        self
    }

    /// Requires all rows to fit in one SQL statement.
    pub fn single_statement(mut self) -> Self {
        self.policy = self.policy.single_statement();
        self
    }

    /// Returns every statement plan and its input row range.
    pub fn plans(&self) -> Result<BatchPlan, QueryError> {
        self.as_borrowed().plans()
    }

    /// Renders SQL and parameter metadata without executing the insert.
    pub fn plan(&self) -> Result<QueryPlan, QueryError> {
        self.as_borrowed().plan()
    }

    /// Executes this owned batch insert.
    pub async fn exec<S>(self, session: &mut S) -> Result<u64, DbError>
    where
        S: DbSession,
    {
        BatchInsert {
            scope: self.scope,
            rows: &self.rows,
            policy: self.policy,
            conflict: self.conflict,
        }
        .exec(session)
        .await
    }

    fn as_borrowed(&self) -> BatchInsert<'_, T> {
        BatchInsert {
            scope: self.scope.clone(),
            rows: &self.rows,
            policy: self.policy,
            conflict: self.conflict.clone(),
        }
    }
}

impl<T> OwnedBatchUpsert<T>
where
    T: Record + 'static,
{
    /// Limits the maximum rows rendered into each SQL statement.
    pub fn batch_size(mut self, size: usize) -> Self {
        self.policy = self.policy.with_size(size);
        self
    }

    /// Requires all rows to fit in one SQL statement.
    pub fn single_statement(mut self) -> Self {
        self.policy = self.policy.single_statement();
        self
    }

    /// Restricts the columns changed by conflict updates.
    pub fn update_only<C>(mut self, columns: C) -> Self
    where
        C: ColumnSet,
    {
        self.update_columns = Some(columns.into_column_refs());
        self
    }

    /// Returns every statement plan and its input row range.
    pub fn plans(&self) -> Result<BatchPlan, QueryError> {
        self.as_borrowed().plans()
    }

    /// Renders SQL and parameter metadata without executing the upsert.
    pub fn plan(&self) -> Result<QueryPlan, QueryError> {
        self.as_borrowed().plan()
    }

    /// Executes this owned batch upsert.
    pub async fn exec<S>(self, session: &mut S) -> Result<u64, DbError>
    where
        S: DbSession,
    {
        BatchUpsert {
            scope: self.scope,
            rows: &self.rows,
            conflict: self.conflict,
            update_columns: self.update_columns,
            policy: self.policy,
        }
        .exec(session)
        .await
    }

    fn as_borrowed(&self) -> BatchUpsert<'_, T> {
        BatchUpsert {
            scope: self.scope.clone(),
            rows: &self.rows,
            conflict: self.conflict.clone(),
            update_columns: self.update_columns.clone(),
            policy: self.policy,
        }
    }
}

impl<T> OwnedBatchUpdate<T>
where
    T: Model + 'static,
{
    /// Limits the maximum rows rendered into each SQL statement.
    pub fn batch_size(mut self, size: usize) -> Self {
        self.policy = self.policy.with_size(size);
        self
    }

    /// Requires all rows to fit in one SQL statement.
    pub fn single_statement(mut self) -> Self {
        self.policy = self.policy.single_statement();
        self
    }

    /// Returns every statement plan and its input row range.
    pub fn plans(&self) -> Result<BatchPlan, QueryError> {
        self.as_borrowed().plans()
    }

    /// Renders one SQL statement when the operation fits one batch.
    pub fn plan(&self) -> Result<QueryPlan, QueryError> {
        self.as_borrowed().plan()
    }

    /// Executes this owned batch update.
    pub async fn exec<S>(self, session: &mut S) -> Result<u64, DbError>
    where
        S: DbSession,
    {
        BatchUpdate {
            scope: self.scope,
            rows: &self.rows,
            update_columns: self.update_columns,
            policy: self.policy,
        }
        .exec(session)
        .await
    }

    fn as_borrowed(&self) -> BatchUpdate<'_, T> {
        BatchUpdate {
            scope: self.scope.clone(),
            rows: &self.rows,
            update_columns: self.update_columns.clone(),
            policy: self.policy,
        }
    }
}
