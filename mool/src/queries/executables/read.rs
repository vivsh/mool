//! Read-row executable implementations.

use crate::commons::{Arguments, Row};
use crate::executor::{DBSession, DbError};
use crate::interfaces::Record;
use crate::placeholders::Dialect;

use super::super::binds::statement_from_plan;
use super::super::expr::IntoExpr;
use super::super::output::{HasOutputCols, IntoOutputTarget, ReadUsing, select_assignment};
use super::super::plan::QueryPlan;
use super::super::set::{Set, SetOp};
use super::super::source::{Cte, Subquery};
use super::super::traits::Projectable;
use super::{All, First, One, Slice};
use crate::QueryError;

impl<T> All<T>
where
    T: Record + 'static,
{
    /// Assigns a typed expression to a selected output field.
    pub fn set<V>(mut self, target: impl IntoOutputTarget<V>, expr: impl IntoExpr<V>) -> Self
    where
        T: HasOutputCols,
    {
        self.scope
            .output_assignments
            .push(select_assignment(target, expr));
        self
    }

    /// Adds computed expressions to the selected output projection.
    #[doc(hidden)]
    pub fn using<F>(mut self, f: F) -> Self
    where
        T: HasOutputCols,
        F: FnOnce(ReadUsing<T>) -> ReadUsing<T>,
    {
        self.scope.output_assignments = f(ReadUsing::new()).into_selects().assignments;
        self
    }

    /// Renders SQL and parameter metadata without executing the query.
    pub fn plan(&self, dialect: Dialect) -> Result<QueryPlan, QueryError> {
        self.scope.plan_all::<T>(dialect)
    }

    /// Combines this query with another query using `UNION`.
    pub fn union(self, rhs: All<T>) -> Set<T> {
        Set::new(self.scope, rhs.scope, SetOp::Union)
    }

    /// Combines this query with another query using `UNION ALL`.
    pub fn union_all(self, rhs: All<T>) -> Set<T> {
        Set::new(self.scope, rhs.scope, SetOp::UnionAll)
    }

    /// Combines this query with another query using `EXCEPT`.
    pub fn except(self, rhs: All<T>) -> Set<T> {
        Set::new(self.scope, rhs.scope, SetOp::Except)
    }

    /// Converts this select executable into a CTE source.
    pub fn cte(self) -> Result<Cte<T>, QueryError>
    where
        T: Projectable + 'static,
    {
        self.scope.cte::<T>()
    }

    /// Converts this select executable into a named CTE source.
    #[doc(hidden)]
    pub fn cte_as(self, name: &str) -> Result<Cte<T>, QueryError>
    where
        T: Projectable + 'static,
    {
        self.scope.cte_as::<T>(name)
    }

    /// Converts this select executable into a subquery source.
    pub fn subquery(self) -> Result<Subquery<T>, QueryError>
    where
        T: Projectable + 'static,
    {
        self.scope.subquery::<T>()
    }

    /// Converts this select executable into a named subquery source.
    #[doc(hidden)]
    pub fn subquery_as(self, name: &str) -> Result<Subquery<T>, QueryError>
    where
        T: Projectable + 'static,
    {
        self.scope.subquery_as::<T>(name)
    }

    /// Executes this query against a database session.
    pub async fn exec<S>(self, session: &mut S) -> Result<Vec<T>, DbError>
    where
        T: for<'r> sqlx::FromRow<'r, Row> + Send + Unpin + 'static,
        S: DBSession,
    {
        let stmt = statement_from_plan(self.plan(Dialect::active())?, Arguments::default())?;
        session.fetch_all(stmt).await
    }
}

impl<T> One<T>
where
    T: Record + 'static,
{
    /// Assigns a typed expression to a selected output field.
    pub fn set<V>(mut self, target: impl IntoOutputTarget<V>, expr: impl IntoExpr<V>) -> Self
    where
        T: HasOutputCols,
    {
        self.scope
            .output_assignments
            .push(select_assignment(target, expr));
        self
    }

    /// Adds computed expressions to the selected output projection.
    #[doc(hidden)]
    pub fn using<F>(mut self, f: F) -> Self
    where
        T: HasOutputCols,
        F: FnOnce(ReadUsing<T>) -> ReadUsing<T>,
    {
        self.scope.output_assignments = f(ReadUsing::new()).into_selects().assignments;
        self
    }

    /// Renders SQL and parameter metadata without executing the query.
    pub fn plan(&self, dialect: Dialect) -> Result<QueryPlan, QueryError> {
        self.scope.plan_first::<T>(dialect)
    }

    /// Executes this query and requires exactly one row.
    pub async fn exec<S>(self, session: &mut S) -> Result<T, DbError>
    where
        T: for<'r> sqlx::FromRow<'r, Row> + Send + Unpin + 'static,
        S: DBSession,
    {
        let stmt = statement_from_plan(self.plan(Dialect::active())?, Arguments::default())?;
        session.fetch_one(stmt).await
    }
}

impl<T> First<T>
where
    T: Record + 'static,
{
    /// Assigns a typed expression to a selected output field.
    pub fn set<V>(mut self, target: impl IntoOutputTarget<V>, expr: impl IntoExpr<V>) -> Self
    where
        T: HasOutputCols,
    {
        self.scope
            .output_assignments
            .push(select_assignment(target, expr));
        self
    }

    /// Adds computed expressions to the selected output projection.
    #[doc(hidden)]
    pub fn using<F>(mut self, f: F) -> Self
    where
        T: HasOutputCols,
        F: FnOnce(ReadUsing<T>) -> ReadUsing<T>,
    {
        self.scope.output_assignments = f(ReadUsing::new()).into_selects().assignments;
        self
    }

    /// Renders SQL and parameter metadata without executing the query.
    pub fn plan(&self, dialect: Dialect) -> Result<QueryPlan, QueryError> {
        self.scope.plan_first::<T>(dialect)
    }

    /// Executes this query and returns the first row, if any.
    pub async fn exec<S>(self, session: &mut S) -> Result<Option<T>, DbError>
    where
        T: for<'r> sqlx::FromRow<'r, Row> + Send + Unpin + 'static,
        S: DBSession,
    {
        let stmt = statement_from_plan(self.plan(Dialect::active())?, Arguments::default())?;
        session.fetch_optional(stmt).await
    }
}

impl<T> Slice<T>
where
    T: Record + 'static,
{
    /// Assigns a typed expression to a selected output field.
    pub fn set<V>(mut self, target: impl IntoOutputTarget<V>, expr: impl IntoExpr<V>) -> Self
    where
        T: HasOutputCols,
    {
        self.scope
            .output_assignments
            .push(select_assignment(target, expr));
        self
    }

    /// Adds computed expressions to the selected output projection.
    #[doc(hidden)]
    pub fn using<F>(mut self, f: F) -> Self
    where
        T: HasOutputCols,
        F: FnOnce(ReadUsing<T>) -> ReadUsing<T>,
    {
        self.scope.output_assignments = f(ReadUsing::new()).into_selects().assignments;
        self
    }

    /// Renders SQL and parameter metadata without executing the query.
    pub fn plan(&self, dialect: Dialect) -> Result<QueryPlan, QueryError> {
        self.scope.plan_slice::<T>(self.offset, self.count, dialect)
    }

    /// Converts this limited select executable into a CTE source.
    pub fn cte(self) -> Result<Cte<T>, QueryError>
    where
        T: Projectable + 'static,
    {
        self.scope.cte_as_slice::<T>(
            &super::super::validate::generated_source_name::<T>("cte"),
            Some((self.offset, self.count)),
        )
    }

    /// Converts this limited select executable into a subquery source.
    pub fn subquery(self) -> Result<Subquery<T>, QueryError>
    where
        T: Projectable,
    {
        self.scope.subquery_as_slice::<T>(
            &super::super::validate::generated_source_name::<T>("subquery"),
            Some((self.offset, self.count)),
        )
    }

    /// Executes this limited query against a database session.
    pub async fn exec<S>(self, session: &mut S) -> Result<Vec<T>, DbError>
    where
        T: for<'r> sqlx::FromRow<'r, Row> + Send + Unpin + 'static,
        S: DBSession,
    {
        let stmt = statement_from_plan(self.plan(Dialect::active())?, Arguments::default())?;
        session.fetch_all(stmt).await
    }
}
