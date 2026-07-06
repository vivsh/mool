//! Returning write executable implementations.

use crate::commons::{Arguments, Row};
use crate::executor::{DBSession, DbError};
use crate::interfaces::Record;
use crate::placeholders::Dialect;

use super::super::binds::statement_from_plan;
use super::super::expr::IntoExpr;
use super::super::handles::{Column, Var};
use super::super::plan::QueryPlan;
use super::super::values::{WriteInput, WriteUsing, WriteValues};
use super::{
    ReturningBatchInsert, ReturningBatchUpsert, ReturningDelete, ReturningInsert, ReturningUpdate,
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
    pub fn plan(&self, dialect: Dialect) -> Result<QueryPlan, QueryError> {
        self.returning
            .plan_insert(&self.row, dialect)
            .map(|(plan, _)| plan)
    }

    /// Executes this returning insert.
    pub async fn exec<S>(self, session: &mut S) -> Result<R, DbError>
    where
        R: for<'r> sqlx::FromRow<'r, Row> + Send + Unpin + 'static,
        S: DBSession,
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

    /// Adds computed write assignments on top of the record payload.
    #[doc(hidden)]
    pub fn using<F>(self, f: F) -> ReturningInsert<R, WriteValues<'a, T>>
    where
        F: FnOnce(WriteUsing) -> WriteUsing,
    {
        ReturningInsert {
            returning: self.returning,
            row: WriteValues::record(self.row).extend(f(WriteUsing::new()).into_values()),
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
    pub fn plan(&self, dialect: Dialect) -> Result<QueryPlan, QueryError> {
        self.returning
            .plan_update(&self.row, dialect)
            .map(|(plan, _)| plan)
    }

    /// Executes this returning update.
    pub async fn exec<S>(self, session: &mut S) -> Result<Vec<R>, DbError>
    where
        R: for<'r> sqlx::FromRow<'r, Row> + Send + Unpin + 'static,
        S: DBSession,
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

    /// Adds computed write assignments on top of the record payload.
    #[doc(hidden)]
    pub fn using<F>(self, f: F) -> ReturningUpdate<R, WriteValues<'a, T>>
    where
        F: FnOnce(WriteUsing) -> WriteUsing,
    {
        ReturningUpdate {
            returning: self.returning,
            row: WriteValues::record(self.row).extend(f(WriteUsing::new()).into_values()),
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
    pub fn plan(&self, dialect: Dialect) -> Result<QueryPlan, QueryError> {
        self.returning.plan_delete(dialect)
    }

    /// Executes this returning delete.
    pub async fn exec<S>(self, session: &mut S) -> Result<Vec<R>, DbError>
    where
        R: for<'r> sqlx::FromRow<'r, Row> + Send + Unpin + 'static,
        S: DBSession,
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
    /// Renders SQL and parameter metadata without executing the insert.
    pub fn plan(&self, dialect: Dialect) -> Result<QueryPlan, QueryError> {
        self.returning
            .plan_batch_insert(self.rows, dialect)
            .map(|(plan, _)| plan)
    }

    /// Executes this returning batch insert.
    pub async fn exec<S>(self, session: &mut S) -> Result<Vec<R>, DbError>
    where
        R: for<'r> sqlx::FromRow<'r, Row> + Send + Unpin + 'static,
        S: DBSession,
    {
        let (plan, args) = self
            .returning
            .plan_batch_insert(self.rows, Dialect::active())?;
        session.fetch_all(statement_from_plan(plan, args)?).await
    }
}

impl<R, T> ReturningBatchUpsert<'_, R, T>
where
    R: Record + 'static,
    T: Record + 'static,
{
    /// Renders SQL and parameter metadata without executing the upsert.
    pub fn plan(&self, dialect: Dialect) -> Result<QueryPlan, QueryError> {
        self.returning
            .plan_batch_upsert(self.rows, self.conflict.clone(), dialect)
            .map(|(plan, _)| plan)
    }

    /// Executes this returning batch upsert.
    pub async fn exec<S>(self, session: &mut S) -> Result<Vec<R>, DbError>
    where
        R: for<'r> sqlx::FromRow<'r, Row> + Send + Unpin + 'static,
        S: DBSession,
    {
        let (plan, args) =
            self.returning
                .plan_batch_upsert(self.rows, self.conflict, Dialect::active())?;
        session.fetch_all(statement_from_plan(plan, args)?).await
    }
}
