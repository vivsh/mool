//! PostgreSQL `UNNEST` batch executable implementations.

use crate::QueryError;
use crate::backend::PgBatchColumns;
use crate::commons::Row;
use crate::executor::{DbError, DbSession};
use crate::interfaces::{BatchRecord, Record};
use crate::placeholders::Dialect;
use crate::queries::ColumnSet;
use crate::queries::batch::{BatchPlan, BatchStatementPlan};
use crate::queries::binds::statement_from_plan;

use super::super::plan::QueryPlan;
use super::{
    BatchInsert, BatchUpsert, OwnedPgUnnestBatchInsert, OwnedPgUnnestBatchUpsert,
    PgUnnestBatchInsert, PgUnnestBatchUpsert, ReturningPgUnnestBatchInsert,
    ReturningPgUnnestBatchUpsert,
};

impl<T> PgUnnestBatchInsert<'_, T>
where
    T: BatchRecord + 'static,
    T::BatchColumns: PgBatchColumns,
{
    /// Limits the maximum rows transposed into each array statement.
    pub fn batch_size(mut self, size: usize) -> Self {
        self.inner.policy = self.inner.policy.with_size(size);
        self
    }

    /// Requires the complete input to use one `UNNEST` statement.
    pub fn single_statement(mut self) -> Self {
        self.inner.policy = self.inner.policy.single_statement();
        self
    }

    /// Returns every `UNNEST` statement plan and its input row range.
    pub fn plans(&self) -> Result<BatchPlan, QueryError> {
        let ranges = self
            .inner
            .policy
            .unnest_ranges("PostgreSQL UNNEST insert", self.inner.rows.len())?;
        let mode = self.inner.mode();
        let mut statements = Vec::with_capacity(ranges.len());
        for range in ranges {
            let plan = self.inner.scope.plan_batch_unnest::<T>(
                range.len(),
                Dialect::active(),
                &mode,
                None,
            )?;
            statements.push(BatchStatementPlan::new(plan, range));
        }
        Ok(BatchPlan::new(statements))
    }

    /// Renders one SQL statement when the operation has one array batch.
    pub fn plan(&self) -> Result<QueryPlan, QueryError> {
        super::write::one_batch_plan(self.plans()?)
    }

    /// Copies all rows into an owned PostgreSQL `UNNEST` executable.
    pub fn into_owned(self) -> OwnedPgUnnestBatchInsert<T>
    where
        T: Clone,
    {
        OwnedPgUnnestBatchInsert {
            inner: self.inner.into_owned(),
        }
    }

    /// Executes each `UNNEST` statement in input order.
    pub async fn exec<S>(self, session: &mut S) -> Result<u64, DbError>
    where
        S: DbSession,
    {
        let ranges = self
            .inner
            .policy
            .unnest_ranges("PostgreSQL UNNEST insert", self.inner.rows.len())?;
        let mode = self.inner.mode();
        let mut affected = 0_u64;
        for range in ranges {
            let (plan, args) = self.inner.scope.plan_batch_unnest_with_args(
                &self.inner.rows[range],
                Dialect::active(),
                &mode,
                None,
            )?;
            affected = add_affected(
                affected,
                session.execute(statement_from_plan(plan, args)?).await?,
            )?;
        }
        Ok(affected)
    }
}

impl<T> PgUnnestBatchUpsert<'_, T>
where
    T: BatchRecord + 'static,
    T::BatchColumns: PgBatchColumns,
{
    /// Limits the maximum rows transposed into each array statement.
    pub fn batch_size(mut self, size: usize) -> Self {
        self.inner.policy = self.inner.policy.with_size(size);
        self
    }

    /// Requires the complete input to use one `UNNEST` statement.
    pub fn single_statement(mut self) -> Self {
        self.inner.policy = self.inner.policy.single_statement();
        self
    }

    /// Restricts columns changed by conflict updates.
    pub fn update_only<C>(mut self, columns: C) -> Self
    where
        C: ColumnSet,
    {
        self.inner.update_columns = Some(columns.into_column_refs());
        self
    }

    /// Returns every `UNNEST` statement plan and its input row range.
    pub fn plans(&self) -> Result<BatchPlan, QueryError> {
        plan_unnest_upsert(&self.inner)
    }

    /// Renders one SQL statement when the operation has one array batch.
    pub fn plan(&self) -> Result<QueryPlan, QueryError> {
        super::write::one_batch_plan(self.plans()?)
    }

    /// Copies all rows into an owned PostgreSQL `UNNEST` executable.
    pub fn into_owned(self) -> OwnedPgUnnestBatchUpsert<T>
    where
        T: Clone,
    {
        OwnedPgUnnestBatchUpsert {
            inner: self.inner.into_owned(),
        }
    }

    /// Executes each `UNNEST` upsert statement in input order.
    pub async fn exec<S>(self, session: &mut S) -> Result<u64, DbError>
    where
        S: DbSession,
    {
        let ranges = self
            .inner
            .policy
            .unnest_ranges("PostgreSQL UNNEST upsert", self.inner.rows.len())?;
        let mode = self.inner.mode();
        let mut affected = 0_u64;
        for range in ranges {
            let (plan, args) = self.inner.scope.plan_batch_unnest_with_args(
                &self.inner.rows[range],
                Dialect::active(),
                &mode,
                None,
            )?;
            affected = add_affected(
                affected,
                session.execute(statement_from_plan(plan, args)?).await?,
            )?;
        }
        Ok(affected)
    }
}

impl<T> OwnedPgUnnestBatchInsert<T>
where
    T: BatchRecord + 'static,
    T::BatchColumns: PgBatchColumns,
{
    /// Limits the maximum rows transposed into each array statement.
    pub fn batch_size(mut self, size: usize) -> Self {
        self.inner.policy = self.inner.policy.with_size(size);
        self
    }

    /// Requires the complete input to use one `UNNEST` statement.
    pub fn single_statement(mut self) -> Self {
        self.inner.policy = self.inner.policy.single_statement();
        self
    }

    /// Returns every `UNNEST` statement plan and its input row range.
    pub fn plans(&self) -> Result<BatchPlan, QueryError> {
        self.as_borrowed().plans()
    }

    /// Renders one SQL statement when the operation has one array batch.
    pub fn plan(&self) -> Result<QueryPlan, QueryError> {
        self.as_borrowed().plan()
    }

    /// Executes each `UNNEST` statement in input order.
    pub async fn exec<S>(self, session: &mut S) -> Result<u64, DbError>
    where
        S: DbSession,
    {
        let inner = self.inner;
        PgUnnestBatchInsert {
            inner: BatchInsert {
                scope: inner.scope,
                rows: &inner.rows,
                policy: inner.policy,
                conflict: inner.conflict,
            },
        }
        .exec(session)
        .await
    }

    fn as_borrowed(&self) -> PgUnnestBatchInsert<'_, T> {
        PgUnnestBatchInsert {
            inner: BatchInsert {
                scope: self.inner.scope.clone(),
                rows: &self.inner.rows,
                policy: self.inner.policy,
                conflict: self.inner.conflict.clone(),
            },
        }
    }
}

impl<T> OwnedPgUnnestBatchUpsert<T>
where
    T: BatchRecord + 'static,
    T::BatchColumns: PgBatchColumns,
{
    /// Limits the maximum rows transposed into each array statement.
    pub fn batch_size(mut self, size: usize) -> Self {
        self.inner.policy = self.inner.policy.with_size(size);
        self
    }

    /// Requires the complete input to use one `UNNEST` statement.
    pub fn single_statement(mut self) -> Self {
        self.inner.policy = self.inner.policy.single_statement();
        self
    }

    /// Restricts columns changed by conflict updates.
    pub fn update_only<C>(mut self, columns: C) -> Self
    where
        C: ColumnSet,
    {
        self.inner.update_columns = Some(columns.into_column_refs());
        self
    }

    /// Returns every `UNNEST` statement plan and its input row range.
    pub fn plans(&self) -> Result<BatchPlan, QueryError> {
        self.as_borrowed().plans()
    }

    /// Renders one SQL statement when the operation has one array batch.
    pub fn plan(&self) -> Result<QueryPlan, QueryError> {
        self.as_borrowed().plan()
    }

    /// Executes each `UNNEST` upsert statement in input order.
    pub async fn exec<S>(self, session: &mut S) -> Result<u64, DbError>
    where
        S: DbSession,
    {
        let inner = self.inner;
        PgUnnestBatchUpsert {
            inner: BatchUpsert {
                scope: inner.scope,
                rows: &inner.rows,
                conflict: inner.conflict,
                update_columns: inner.update_columns,
                policy: inner.policy,
            },
        }
        .exec(session)
        .await
    }

    fn as_borrowed(&self) -> PgUnnestBatchUpsert<'_, T> {
        PgUnnestBatchUpsert {
            inner: BatchUpsert {
                scope: self.inner.scope.clone(),
                rows: &self.inner.rows,
                conflict: self.inner.conflict.clone(),
                update_columns: self.inner.update_columns.clone(),
                policy: self.inner.policy,
            },
        }
    }
}

impl<R, T> ReturningPgUnnestBatchInsert<'_, R, T>
where
    R: Record + 'static,
    T: BatchRecord + 'static,
    T::BatchColumns: PgBatchColumns,
{
    /// Limits the maximum rows transposed into each returning statement.
    pub fn batch_size(mut self, size: usize) -> Self {
        self.inner.policy = self.inner.policy.with_size(size);
        self
    }

    /// Requires the complete input to use one returning statement.
    pub fn single_statement(mut self) -> Self {
        self.inner.policy = self.inner.policy.single_statement();
        self
    }

    /// Returns every returning `UNNEST` plan and its row range.
    pub fn plans(&self) -> Result<BatchPlan, QueryError> {
        let ranges = self
            .inner
            .policy
            .unnest_ranges("PostgreSQL UNNEST insert", self.inner.rows.len())?;
        let mode = self.inner.mode();
        let mut statements = Vec::with_capacity(ranges.len());
        for range in ranges {
            let plan = self.inner.returning.plan_batch_unnest_shape::<T>(
                range.len(),
                &mode,
                Dialect::active(),
            )?;
            statements.push(BatchStatementPlan::new(plan, range));
        }
        Ok(BatchPlan::new(statements))
    }

    /// Renders one SQL statement when the operation has one array batch.
    pub fn plan(&self) -> Result<QueryPlan, QueryError> {
        super::write::one_batch_plan(self.plans()?)
    }

    /// Executes returning `UNNEST` statements in input order.
    pub async fn exec<S>(self, session: &mut S) -> Result<Vec<R>, DbError>
    where
        R: for<'r> sqlx::FromRow<'r, Row> + Send + Unpin + 'static,
        S: DbSession,
    {
        let ranges = self
            .inner
            .policy
            .unnest_ranges("PostgreSQL UNNEST insert", self.inner.rows.len())?;
        let mode = self.inner.mode();
        let mut output = Vec::new();
        for range in ranges {
            let (plan, args) = self.inner.returning.plan_batch_unnest(
                &self.inner.rows[range],
                &mode,
                Dialect::active(),
            )?;
            output.extend(session.fetch_all(statement_from_plan(plan, args)?).await?);
        }
        Ok(output)
    }
}

impl<R, T> ReturningPgUnnestBatchUpsert<'_, R, T>
where
    R: Record + 'static,
    T: BatchRecord + 'static,
    T::BatchColumns: PgBatchColumns,
{
    /// Limits the maximum rows transposed into each returning statement.
    pub fn batch_size(mut self, size: usize) -> Self {
        self.inner.policy = self.inner.policy.with_size(size);
        self
    }

    /// Requires the complete input to use one returning statement.
    pub fn single_statement(mut self) -> Self {
        self.inner.policy = self.inner.policy.single_statement();
        self
    }

    /// Restricts columns changed by conflict updates.
    pub fn update_only<C>(mut self, columns: C) -> Self
    where
        C: ColumnSet,
    {
        self.inner.update_columns = Some(columns.into_column_refs());
        self
    }

    /// Returns every returning `UNNEST` plan and its row range.
    pub fn plans(&self) -> Result<BatchPlan, QueryError> {
        plan_returning_unnest_upsert(&self.inner)
    }

    /// Renders one SQL statement when the operation has one array batch.
    pub fn plan(&self) -> Result<QueryPlan, QueryError> {
        super::write::one_batch_plan(self.plans()?)
    }

    /// Executes returning `UNNEST` upserts in input order.
    pub async fn exec<S>(self, session: &mut S) -> Result<Vec<R>, DbError>
    where
        R: for<'r> sqlx::FromRow<'r, Row> + Send + Unpin + 'static,
        S: DbSession,
    {
        let ranges = self
            .inner
            .policy
            .unnest_ranges("PostgreSQL UNNEST upsert", self.inner.rows.len())?;
        let mode = self.inner.mode();
        let mut output = Vec::new();
        for range in ranges {
            let (plan, args) = self.inner.returning.plan_batch_unnest(
                &self.inner.rows[range],
                &mode,
                Dialect::active(),
            )?;
            output.extend(session.fetch_all(statement_from_plan(plan, args)?).await?);
        }
        Ok(output)
    }
}

/// Builds all non-returning UNNEST upsert plans in input order.
fn plan_unnest_upsert<T>(batch: &super::BatchUpsert<'_, T>) -> Result<BatchPlan, QueryError>
where
    T: BatchRecord + 'static,
    T::BatchColumns: PgBatchColumns,
{
    let ranges = batch
        .policy
        .unnest_ranges("PostgreSQL UNNEST upsert", batch.rows.len())?;
    let mode = batch.mode();
    let mut statements = Vec::with_capacity(ranges.len());
    for range in ranges {
        let plan =
            batch
                .scope
                .plan_batch_unnest::<T>(range.len(), Dialect::active(), &mode, None)?;
        statements.push(BatchStatementPlan::new(plan, range));
    }
    Ok(BatchPlan::new(statements))
}

/// Builds all returning UNNEST upsert plans in input order.
fn plan_returning_unnest_upsert<R, T>(
    batch: &super::ReturningBatchUpsert<'_, R, T>,
) -> Result<BatchPlan, QueryError>
where
    R: Record + 'static,
    T: BatchRecord + 'static,
    T::BatchColumns: PgBatchColumns,
{
    let ranges = batch
        .policy
        .unnest_ranges("PostgreSQL UNNEST upsert", batch.rows.len())?;
    let mode = batch.mode();
    let mut statements = Vec::with_capacity(ranges.len());
    for range in ranges {
        let plan =
            batch
                .returning
                .plan_batch_unnest_shape::<T>(range.len(), &mode, Dialect::active())?;
        statements.push(BatchStatementPlan::new(plan, range));
    }
    Ok(BatchPlan::new(statements))
}

fn add_affected(total: u64, rows: u64) -> Result<u64, DbError> {
    total.checked_add(rows).ok_or(DbError::AffectedRowsOverflow)
}
