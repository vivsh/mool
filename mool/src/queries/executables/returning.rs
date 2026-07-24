//! Returning write executable implementations.

use crate::commons::{Arguments, Row};
use crate::executor::{DbError, DbSession};
use crate::interfaces::{Model, Record};
use crate::placeholders::Dialect;

use super::super::ColumnSet;
use super::super::batch::{BatchInsertMode, BatchPlan, BatchStatementPlan};
use super::super::binds::statement_from_plan;
use super::super::expr::IntoExpr;
use super::super::handles::{Column, Var};
use super::super::plan::QueryPlan;
use super::super::values::{WriteInput, WriteValues};
use super::{
    ReturningBatchInsert, ReturningBatchUpdate, ReturningBatchUpsert, ReturningDelete,
    ReturningInsert, ReturningUpdate,
};
use crate::QueryError;

impl<R, T> ReturningInsert<R, T>
where
    R: Record + 'static,
    T: WriteInput,
{
    /// Binds a runtime value for a `var(...)` used by this returning insert.
    pub fn bind<V>(mut self, var: &Var<V>, value: V) -> Self
    where
        V: Clone
            + for<'q> sqlx::Encode<'q, crate::commons::Database>
            + sqlx::Type<crate::commons::Database>
            + Send
            + Sync
            + 'static,
    {
        self.returning.scope = self.returning.scope.bind(var, value);
        self
    }

    /// Renders SQL and parameter metadata without executing the insert.
    pub fn plan(&self) -> Result<QueryPlan, QueryError> {
        self.returning
            .plan_insert_shape(&self.row, Dialect::active())
    }

    /// Executes this returning insert.
    pub async fn exec<S>(self, session: &mut S) -> Result<R, DbError>
    where
        R: for<'r> sqlx::FromRow<'r, Row> + Send + Unpin + 'static,
        S: DbSession,
    {
        let (plan, args) = self.returning.plan_insert(&self.row, Dialect::active())?;
        session.fetch_one(statement_from_plan(plan, args)?).await
    }
}

impl<'a, R, T> ReturningInsert<R, &'a T>
where
    R: Record + 'static,
    T: Record,
{
    /// Adds a computed write assignment on top of the record payload.
    pub fn set<V>(
        self,
        column: &Column<V>,
        expr: impl IntoExpr<V>,
    ) -> ReturningInsert<R, WriteValues<'a, T>> {
        ReturningInsert {
            returning: self.returning,
            row: WriteValues::record(self.row).set(column, expr),
        }
    }
}

impl<'a, R, T> ReturningInsert<R, WriteValues<'a, T>>
where
    R: Record + 'static,
{
    /// Adds a computed write assignment.
    pub fn set<V>(mut self, column: &Column<V>, expr: impl IntoExpr<V>) -> Self {
        self.row = self.row.set(column, expr);
        self
    }
}

impl<R, T> ReturningUpdate<R, T>
where
    R: Record + 'static,
    T: WriteInput,
{
    /// Binds a runtime value for a `var(...)` used by this returning update.
    pub fn bind<V>(mut self, var: &Var<V>, value: V) -> Self
    where
        V: Clone
            + for<'q> sqlx::Encode<'q, crate::commons::Database>
            + sqlx::Type<crate::commons::Database>
            + Send
            + Sync
            + 'static,
    {
        self.returning.scope = self.returning.scope.bind(var, value);
        self
    }

    /// Renders SQL and parameter metadata without executing the update.
    pub fn plan(&self) -> Result<QueryPlan, QueryError> {
        self.returning
            .plan_update_shape(&self.row, Dialect::active())
    }

    /// Executes this returning update.
    pub async fn exec<S>(self, session: &mut S) -> Result<Vec<R>, DbError>
    where
        R: for<'r> sqlx::FromRow<'r, Row> + Send + Unpin + 'static,
        S: DbSession,
    {
        let (plan, args) = self.returning.plan_update(&self.row, Dialect::active())?;
        session.fetch_all(statement_from_plan(plan, args)?).await
    }
}

impl<'a, R, T> ReturningUpdate<R, &'a T>
where
    R: Record + 'static,
    T: Record,
{
    /// Adds a computed write assignment on top of the record payload.
    pub fn set<V>(
        self,
        column: &Column<V>,
        expr: impl IntoExpr<V>,
    ) -> ReturningUpdate<R, WriteValues<'a, T>> {
        ReturningUpdate {
            returning: self.returning,
            row: WriteValues::record(self.row).set(column, expr),
        }
    }
}

impl<'a, R, T> ReturningUpdate<R, WriteValues<'a, T>>
where
    R: Record + 'static,
{
    /// Adds a computed write assignment.
    pub fn set<V>(mut self, column: &Column<V>, expr: impl IntoExpr<V>) -> Self {
        self.row = self.row.set(column, expr);
        self
    }
}

impl<R> ReturningDelete<R>
where
    R: Record + 'static,
{
    /// Renders SQL and parameter metadata without executing the delete.
    pub fn plan(&self) -> Result<QueryPlan, QueryError> {
        self.returning.plan_delete(Dialect::active())
    }

    /// Executes this returning delete.
    pub async fn exec<S>(self, session: &mut S) -> Result<Vec<R>, DbError>
    where
        R: for<'r> sqlx::FromRow<'r, Row> + Send + Unpin + 'static,
        S: DbSession,
    {
        let plan = self.returning.plan_delete(Dialect::active())?;
        session
            .fetch_all(statement_from_plan(plan, Arguments::default())?)
            .await
    }
}

impl<R, T> ReturningBatchInsert<'_, R, T>
where
    R: Record + 'static,
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
        let ranges = self.ranges()?;
        let mode = self.mode();
        let mut statements = Vec::with_capacity(ranges.len());
        for range in ranges {
            let plan = self.returning.plan_batch_insert_shape::<T>(
                range.len(),
                &mode,
                Dialect::active(),
            )?;
            statements.push(BatchStatementPlan::new(plan, range));
        }
        Ok(BatchPlan::new(statements))
    }

    /// Renders SQL and parameter metadata without executing the insert.
    pub fn plan(&self) -> Result<QueryPlan, QueryError> {
        super::write::one_batch_plan(self.plans()?)
    }

    /// Executes this returning batch insert.
    pub async fn exec<S>(self, session: &mut S) -> Result<Vec<R>, DbError>
    where
        R: for<'r> sqlx::FromRow<'r, Row> + Send + Unpin + 'static,
        S: DbSession,
    {
        let ranges = self.ranges()?;
        let mode = self.mode();
        let mut output = Vec::new();
        for range in ranges {
            let (plan, args) = self.returning.plan_batch_insert_mode(
                &self.rows[range],
                &mode,
                Dialect::active(),
            )?;
            output.extend(session.fetch_all(statement_from_plan(plan, args)?).await?);
        }
        Ok(output)
    }

    /// Sizes returning insert batches after reserving projection parameters.
    fn ranges(&self) -> Result<Vec<std::ops::Range<usize>>, QueryError> {
        let width = T::record_insert_column_names().len();
        if self.rows.is_empty() {
            return self.policy.ranges("batch insert", 0, width);
        }
        let sample =
            self.returning
                .plan_batch_insert_shape::<T>(1, &self.mode(), Dialect::active())?;
        self.policy.ranges_with_overhead(
            "batch insert",
            self.rows.len(),
            width,
            super::write::fixed_parameter_count(&sample, width)?,
        )
    }

    pub(super) fn mode(&self) -> BatchInsertMode {
        match &self.conflict {
            super::super::InsertConflict::None => BatchInsertMode::Insert,
            #[cfg(any(feature = "postgres", feature = "sqlite"))]
            super::super::InsertConflict::Ignore(columns) => {
                BatchInsertMode::Ignore(columns.clone())
            }
            #[cfg(any(feature = "mysql", feature = "mariadb"))]
            super::super::InsertConflict::IgnoreErrors => BatchInsertMode::IgnoreErrors,
        }
    }
}

impl<R, T> ReturningBatchUpsert<'_, R, T>
where
    R: Record + 'static,
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
        let ranges = self.ranges()?;
        let mode = self.mode();
        let mut statements = Vec::with_capacity(ranges.len());
        for range in ranges {
            let plan = self.returning.plan_batch_insert_shape::<T>(
                range.len(),
                &mode,
                Dialect::active(),
            )?;
            statements.push(BatchStatementPlan::new(plan, range));
        }
        Ok(BatchPlan::new(statements))
    }

    /// Renders SQL and parameter metadata without executing the upsert.
    pub fn plan(&self) -> Result<QueryPlan, QueryError> {
        super::write::one_batch_plan(self.plans()?)
    }

    /// Executes this returning batch upsert.
    pub async fn exec<S>(self, session: &mut S) -> Result<Vec<R>, DbError>
    where
        R: for<'r> sqlx::FromRow<'r, Row> + Send + Unpin + 'static,
        S: DbSession,
    {
        let ranges = self.ranges()?;
        let mode = self.mode();
        let mut output = Vec::new();
        for range in ranges {
            let (plan, args) = self.returning.plan_batch_insert_mode(
                &self.rows[range],
                &mode,
                Dialect::active(),
            )?;
            output.extend(session.fetch_all(statement_from_plan(plan, args)?).await?);
        }
        Ok(output)
    }

    /// Sizes returning upsert batches after reserving projection parameters.
    fn ranges(&self) -> Result<Vec<std::ops::Range<usize>>, QueryError> {
        let width = T::record_insert_column_names().len();
        if self.rows.is_empty() {
            return self.policy.ranges("batch upsert", 0, width);
        }
        let sample =
            self.returning
                .plan_batch_insert_shape::<T>(1, &self.mode(), Dialect::active())?;
        self.policy.ranges_with_overhead(
            "batch upsert",
            self.rows.len(),
            width,
            super::write::fixed_parameter_count(&sample, width)?,
        )
    }

    pub(super) fn mode(&self) -> BatchInsertMode {
        BatchInsertMode::Upsert {
            conflict: self.conflict.clone(),
            update_columns: self.update_columns.clone(),
        }
    }
}

impl<R, T> ReturningBatchUpdate<'_, R, T>
where
    R: Record + 'static,
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
        let ranges = self.ranges()?;
        let mut statements = Vec::with_capacity(ranges.len());
        for range in ranges {
            let plan = self.returning.plan_batch_update_shape(
                &self.rows[range.clone()],
                &self.update_columns,
                Dialect::active(),
            )?;
            statements.push(BatchStatementPlan::new(plan, range));
        }
        Ok(BatchPlan::new(statements))
    }

    /// Renders one SQL statement when the operation fits one batch.
    pub fn plan(&self) -> Result<QueryPlan, QueryError> {
        super::write::one_batch_plan(self.plans()?)
    }

    /// Executes each statement and concatenates returned rows in batch order.
    pub async fn exec<S>(self, session: &mut S) -> Result<Vec<R>, DbError>
    where
        R: for<'r> sqlx::FromRow<'r, Row> + Send + Unpin + 'static,
        S: DbSession,
    {
        let ranges = self.ranges()?;
        let mut output = Vec::new();
        for range in ranges {
            let (plan, args) = self.returning.plan_batch_update(
                &self.rows[range],
                &self.update_columns,
                Dialect::active(),
            )?;
            output.extend(session.fetch_all(statement_from_plan(plan, args)?).await?);
        }
        Ok(output)
    }

    /// Sizes returning updates after reserving filter and projection parameters.
    fn ranges(&self) -> Result<Vec<std::ops::Range<usize>>, QueryError> {
        self.returning
            .scope
            .validate_batch_update_input::<T>(self.rows, &self.update_columns)?;
        let width = T::primary_key_columns().len() + self.update_columns.len();
        if self.rows.is_empty() {
            return self.policy.ranges("batch update", 0, width);
        }
        let sample = self.returning.plan_batch_update_shape(
            &self.rows[..1],
            &self.update_columns,
            Dialect::active(),
        )?;
        self.policy.ranges_with_overhead(
            "batch update",
            self.rows.len(),
            width,
            super::write::fixed_parameter_count(&sample, width)?,
        )
    }
}
