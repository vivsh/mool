//! Non-returning write executable implementations.

use crate::commons::Arguments;
use crate::executor::{DBSession, DbError};
use crate::interfaces::Record;
use crate::placeholders::Dialect;

use super::super::binds::statement_from_plan;
use super::super::expr::IntoExpr;
use super::super::handles::{Column, Var};
use super::super::plan::QueryPlan;
use super::super::values::{WriteInput, WriteUsing, WriteValues};
use super::{
    BatchInsert, BatchUpsert, Delete, Insert, OwnedBatchInsert, OwnedBatchUpsert, OwnedInsert,
    OwnedUpdate, Update,
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
    pub fn plan(&self, dialect: Dialect) -> Result<QueryPlan, QueryError> {
        self.scope.plan_insert(&self.row, dialect)
    }

    /// Copies the payload into an owned executable.
    /// Executes this insert.
    pub async fn exec<S>(self, session: &mut S) -> Result<u64, DbError>
    where
        S: DBSession,
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
    pub fn plan(&self, dialect: Dialect) -> Result<QueryPlan, QueryError> {
        self.scope.plan_update(&self.row, dialect)
    }

    /// Copies the payload into an owned executable.
    /// Executes this update.
    pub async fn exec<S>(self, session: &mut S) -> Result<u64, DbError>
    where
        S: DBSession,
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
    pub fn plan(&self, dialect: Dialect) -> Result<QueryPlan, QueryError> {
        self.scope.plan_delete(dialect)
    }

    /// Owned conversion for API symmetry.
    pub fn into_owned(self) -> Self {
        self
    }

    /// Executes this delete.
    pub async fn exec<S>(self, session: &mut S) -> Result<u64, DbError>
    where
        S: DBSession,
    {
        let stmt = statement_from_plan(self.plan(Dialect::active())?, Arguments::default())?;
        session.execute(stmt).await
    }
}

impl<T> BatchInsert<'_, T>
where
    T: Record + 'static,
{
    /// Renders SQL and parameter metadata without executing the insert.
    pub fn plan(&self, dialect: Dialect) -> Result<QueryPlan, QueryError> {
        self.scope.plan_batch_insert(self.rows, dialect)
    }

    /// Copies all rows into an owned executable.
    pub fn into_owned(self) -> OwnedBatchInsert<T>
    where
        T: Clone,
    {
        OwnedBatchInsert {
            scope: self.scope,
            rows: self.rows.to_vec(),
        }
    }

    /// Executes this batch insert.
    pub async fn exec<S>(self, session: &mut S) -> Result<u64, DbError>
    where
        S: DBSession,
    {
        let (plan, args) = self
            .scope
            .plan_batch_insert_with_args(self.rows, Dialect::active())?;
        session.execute(statement_from_plan(plan, args)?).await
    }
}

impl<T> BatchUpsert<'_, T>
where
    T: Record + 'static,
{
    /// Renders SQL and parameter metadata without executing the upsert.
    pub fn plan(&self, dialect: Dialect) -> Result<QueryPlan, QueryError> {
        self.scope
            .plan_batch_upsert(self.rows, self.conflict.clone(), dialect)
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
        }
    }

    /// Executes this batch upsert.
    pub async fn exec<S>(self, session: &mut S) -> Result<u64, DbError>
    where
        S: DBSession,
    {
        let (plan, args) =
            self.scope
                .plan_batch_upsert_with_args(self.rows, self.conflict, Dialect::active())?;
        session.execute(statement_from_plan(plan, args)?).await
    }
}
