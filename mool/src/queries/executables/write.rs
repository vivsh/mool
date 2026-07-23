//! Non-returning write executable implementations.

use crate::commons::Arguments;
use crate::executor::{DbError, DbSession};
use crate::interfaces::{Model, Record};
use crate::placeholders::Dialect;

use super::super::ColumnSet;
use super::super::batch::{BatchInsertMode, BatchPlan, BatchStatementPlan};
use super::super::binds::statement_from_plan;
use super::super::expr::IntoExpr;
use super::super::handles::{Column, Var};
use super::super::plan::QueryPlan;
use super::super::values::{WriteInput, WriteUsing, WriteValues};
use super::{
    BatchInsert, BatchUpdate, BatchUpsert, Delete, Insert, OwnedBatchInsert, OwnedBatchUpdate,
    OwnedBatchUpsert, OwnedInsert, OwnedUpdate, Update,
};
use crate::QueryError;

impl<T> Insert<T>
where
    T: WriteInput,
{
    /// Binds a runtime value for a `var(...)` used by this insert.
    pub fn bind<V>(mut self, var: &Var<V>, value: V) -> Self
    where
        V: Clone
            + for<'q> sqlx::Encode<'q, crate::commons::Database>
            + sqlx::Type<crate::commons::Database>
            + Send
            + Sync
            + 'static,
    {
        self.scope = self.scope.bind(var, value);
        self
    }

    /// Renders SQL and parameter metadata without executing the insert.
    pub fn plan(&self) -> Result<QueryPlan, QueryError> {
        self.scope.plan_insert(&self.row, Dialect::active())
    }

    /// Copies the payload into an owned executable.
    /// Executes this insert.
    pub async fn exec<S>(self, session: &mut S) -> Result<u64, DbError>
    where
        S: DbSession,
    {
        let (plan, args) = self
            .scope
            .plan_insert_with_args(&self.row, Dialect::active())?;
        session.execute(statement_from_plan(plan, args)?).await
    }
}

impl<'a, T> Insert<&'a T>
where
    T: Record,
{
    /// Adds a computed write assignment on top of the record payload.
    pub fn set<V>(self, column: &Column<V>, expr: impl IntoExpr<V>) -> Insert<WriteValues<'a, T>> {
        Insert {
            scope: self.scope,
            row: WriteValues::record(self.row).set(column, expr),
        }
    }

    /// Adds computed write assignments on top of the record payload.
    #[doc(hidden)]
    pub fn using<F>(self, f: F) -> Insert<WriteValues<'a, T>>
    where
        F: FnOnce(WriteUsing) -> WriteUsing,
    {
        Insert {
            scope: self.scope,
            row: WriteValues::record(self.row).extend(f(WriteUsing::new()).into_values()),
        }
    }

    /// Copies the record payload into an owned executable.
    pub fn into_owned(self) -> OwnedInsert<T>
    where
        T: Clone,
    {
        OwnedInsert {
            scope: self.scope,
            row: self.row.clone(),
        }
    }
}

impl<'a, T> Insert<WriteValues<'a, T>> {
    /// Adds a computed write assignment.
    pub fn set<V>(mut self, column: &Column<V>, expr: impl IntoExpr<V>) -> Self {
        self.row = self.row.set(column, expr);
        self
    }
}

impl<T> Update<T>
where
    T: WriteInput,
{
    /// Binds a runtime value for a `var(...)` used by this update.
    pub fn bind<V>(mut self, var: &Var<V>, value: V) -> Self
    where
        V: Clone
            + for<'q> sqlx::Encode<'q, crate::commons::Database>
            + sqlx::Type<crate::commons::Database>
            + Send
            + Sync
            + 'static,
    {
        self.scope = self.scope.bind(var, value);
        self
    }

    /// Renders SQL and parameter metadata without executing the update.
    pub fn plan(&self) -> Result<QueryPlan, QueryError> {
        self.scope.plan_update(&self.row, Dialect::active())
    }

    /// Copies the payload into an owned executable.
    /// Executes this update.
    pub async fn exec<S>(self, session: &mut S) -> Result<u64, DbError>
    where
        S: DbSession,
    {
        let (plan, args) = self
            .scope
            .plan_update_with_args(&self.row, Dialect::active())?;
        session.execute(statement_from_plan(plan, args)?).await
    }
}

impl<'a, T> Update<&'a T>
where
    T: Record,
{
    /// Adds a computed write assignment on top of the record payload.
    pub fn set<V>(self, column: &Column<V>, expr: impl IntoExpr<V>) -> Update<WriteValues<'a, T>> {
        Update {
            scope: self.scope,
            row: WriteValues::record(self.row).set(column, expr),
        }
    }

    /// Adds computed write assignments on top of the record payload.
    #[doc(hidden)]
    pub fn using<F>(self, f: F) -> Update<WriteValues<'a, T>>
    where
        F: FnOnce(WriteUsing) -> WriteUsing,
    {
        Update {
            scope: self.scope,
            row: WriteValues::record(self.row).extend(f(WriteUsing::new()).into_values()),
        }
    }

    /// Copies the record payload into an owned executable.
    pub fn into_owned(self) -> OwnedUpdate<T>
    where
        T: Clone,
    {
        OwnedUpdate {
            scope: self.scope,
            row: self.row.clone(),
        }
    }
}

impl<'a, T> Update<WriteValues<'a, T>> {
    /// Adds a computed write assignment.
    pub fn set<V>(mut self, column: &Column<V>, expr: impl IntoExpr<V>) -> Self {
        self.row = self.row.set(column, expr);
        self
    }
}

impl Delete {
    /// Renders SQL and parameter metadata without executing the delete.
    pub fn plan(&self) -> Result<QueryPlan, QueryError> {
        self.scope.plan_delete(Dialect::active())
    }

    /// Owned conversion for API symmetry.
    pub fn into_owned(self) -> Self {
        self
    }

    /// Executes this delete.
    pub async fn exec<S>(self, session: &mut S) -> Result<u64, DbError>
    where
        S: DbSession,
    {
        let stmt = statement_from_plan(self.plan()?, Arguments::default())?;
        session.execute(stmt).await
    }
}

impl<T> BatchInsert<'_, T>
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
        let columns = T::record_bind_column_names().len();
        let ranges = self
            .policy
            .ranges("batch insert", self.rows.len(), columns)?;
        let mode = self.mode();
        let mut statements = Vec::with_capacity(ranges.len());
        for range in ranges {
            let (plan, _) = self.scope.plan_batch_insert_mode_with_args(
                &self.rows[range.clone()],
                Dialect::active(),
                &mode,
                None,
            )?;
            statements.push(BatchStatementPlan::new(plan, range));
        }
        Ok(BatchPlan::new(statements))
    }

    /// Renders SQL and parameter metadata without executing the insert.
    pub fn plan(&self) -> Result<QueryPlan, QueryError> {
        one_batch_plan(self.plans()?)
    }

    /// Copies all rows into an owned executable.
    pub fn into_owned(self) -> OwnedBatchInsert<T>
    where
        T: Clone,
    {
        OwnedBatchInsert {
            scope: self.scope,
            rows: self.rows.to_vec(),
            policy: self.policy,
            conflict: self.conflict,
        }
    }

    /// Executes this batch insert.
    pub async fn exec<S>(self, session: &mut S) -> Result<u64, DbError>
    where
        S: DbSession,
    {
        let columns = T::record_bind_column_names().len();
        let ranges = self
            .policy
            .ranges("batch insert", self.rows.len(), columns)?;
        let mode = self.mode();
        let mut affected = 0_u64;
        for range in ranges {
            let (plan, args) = self.scope.plan_batch_insert_mode_with_args(
                &self.rows[range],
                Dialect::active(),
                &mode,
                None,
            )?;
            let rows_affected = session.execute(statement_from_plan(plan, args)?).await?;
            affected = affected
                .checked_add(rows_affected)
                .ok_or(DbError::AffectedRowsOverflow)?;
        }
        Ok(affected)
    }

    pub(super) fn mode(&self) -> BatchInsertMode {
        match &self.conflict {
            super::super::batch::InsertConflict::None => BatchInsertMode::Insert,
            #[cfg(any(feature = "postgres", feature = "sqlite"))]
            super::super::batch::InsertConflict::Ignore(columns) => {
                BatchInsertMode::Ignore(columns.clone())
            }
            #[cfg(any(feature = "mysql", feature = "mariadb"))]
            super::super::batch::InsertConflict::IgnoreErrors => BatchInsertMode::IgnoreErrors,
        }
    }
}

impl<T> BatchUpsert<'_, T>
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
        let columns = T::record_bind_column_names().len();
        let ranges = self
            .policy
            .ranges("batch upsert", self.rows.len(), columns)?;
        let mode = self.mode();
        let mut statements = Vec::with_capacity(ranges.len());
        for range in ranges {
            let (plan, _) = self.scope.plan_batch_insert_mode_with_args(
                &self.rows[range.clone()],
                Dialect::active(),
                &mode,
                None,
            )?;
            statements.push(BatchStatementPlan::new(plan, range));
        }
        Ok(BatchPlan::new(statements))
    }

    /// Renders SQL and parameter metadata without executing the upsert.
    pub fn plan(&self) -> Result<QueryPlan, QueryError> {
        one_batch_plan(self.plans()?)
    }

    /// Copies all rows into an owned executable.
    pub fn into_owned(self) -> OwnedBatchUpsert<T>
    where
        T: Clone,
    {
        OwnedBatchUpsert {
            scope: self.scope,
            rows: self.rows.to_vec(),
            conflict: self.conflict,
            update_columns: self.update_columns,
            policy: self.policy,
        }
    }

    /// Executes this batch upsert.
    pub async fn exec<S>(self, session: &mut S) -> Result<u64, DbError>
    where
        S: DbSession,
    {
        let columns = T::record_bind_column_names().len();
        let ranges = self
            .policy
            .ranges("batch upsert", self.rows.len(), columns)?;
        let mode = self.mode();
        let mut affected = 0_u64;
        for range in ranges {
            let (plan, args) = self.scope.plan_batch_insert_mode_with_args(
                &self.rows[range],
                Dialect::active(),
                &mode,
                None,
            )?;
            let rows_affected = session.execute(statement_from_plan(plan, args)?).await?;
            affected = affected
                .checked_add(rows_affected)
                .ok_or(DbError::AffectedRowsOverflow)?;
        }
        Ok(affected)
    }

    pub(super) fn mode(&self) -> BatchInsertMode {
        BatchInsertMode::Upsert {
            conflict: self.conflict.clone(),
            update_columns: self.update_columns.clone(),
        }
    }
}

impl<T> BatchUpdate<'_, T>
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
        let ranges = self.ranges()?;
        let mut statements = Vec::with_capacity(ranges.len());
        for range in ranges {
            let (plan, _) = self.scope.plan_batch_update_with_args(
                &self.rows[range.clone()],
                &self.update_columns,
                Dialect::active(),
                None,
            )?;
            statements.push(BatchStatementPlan::new(plan, range));
        }
        Ok(BatchPlan::new(statements))
    }

    /// Renders one SQL statement when the operation fits one batch.
    pub fn plan(&self) -> Result<QueryPlan, QueryError> {
        one_batch_plan(self.plans()?)
    }

    /// Copies all rows into an owned executable.
    pub fn into_owned(self) -> OwnedBatchUpdate<T>
    where
        T: Clone,
    {
        OwnedBatchUpdate {
            scope: self.scope,
            rows: self.rows.to_vec(),
            update_columns: self.update_columns,
            policy: self.policy,
        }
    }

    /// Executes each planned update statement in input order.
    pub async fn exec<S>(self, session: &mut S) -> Result<u64, DbError>
    where
        S: DbSession,
    {
        let ranges = self.ranges()?;
        let mut affected = 0_u64;
        for range in ranges {
            let (plan, args) = self.scope.plan_batch_update_with_args(
                &self.rows[range],
                &self.update_columns,
                Dialect::active(),
                None,
            )?;
            let rows_affected = session.execute(statement_from_plan(plan, args)?).await?;
            affected = affected
                .checked_add(rows_affected)
                .ok_or(DbError::AffectedRowsOverflow)?;
        }
        Ok(affected)
    }

    /// Sizes update batches after reserving parameters used by scope filters.
    fn ranges(&self) -> Result<Vec<std::ops::Range<usize>>, QueryError> {
        self.scope
            .validate_batch_update_input::<T>(self.rows, &self.update_columns)?;
        let width = T::primary_key_columns().len() + self.update_columns.len();
        let Some(first) = self.rows.first() else {
            return self.policy.ranges("batch update", 0, width);
        };
        let (sample, _) = self.scope.plan_batch_update_with_args(
            std::slice::from_ref(first),
            &self.update_columns,
            Dialect::active(),
            None,
        )?;
        self.policy.ranges_with_overhead(
            "batch update",
            self.rows.len(),
            width,
            fixed_parameter_count(&sample, width)?,
        )
    }
}

pub(super) fn one_batch_plan(plan: BatchPlan) -> Result<QueryPlan, QueryError> {
    let count = plan.statements().len();
    if count != 1 {
        return Err(QueryError::MultipleStatementsRequired { statements: count });
    }
    let mut statements = plan.into_statements();
    statements
        .pop()
        .map(|statement| statement.plan().clone())
        .ok_or(QueryError::EmptyBatch {
            operation: "batch operation",
        })
}

/// Returns parameters used outside one row payload in a representative plan.
pub(super) fn fixed_parameter_count(
    plan: &QueryPlan,
    row_parameters: usize,
) -> Result<usize, QueryError> {
    plan.total_bind_count
        .checked_sub(row_parameters)
        .ok_or(QueryError::BindCountMismatch {
            expected: row_parameters,
            got: plan.total_bind_count,
        })
}
