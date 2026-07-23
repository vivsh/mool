//! Scalar and aggregate executable implementations.

use crate::commons::Arguments;
use crate::executor::{DbError, DbSession};
use crate::placeholders::Dialect;

use super::super::binds::statement_from_plan;
use super::super::plan::QueryPlan;
use super::{Count, Exists, Scalar};
use crate::QueryError;

impl Count {
    /// Renders SQL and parameter metadata without executing the query.
    pub fn plan(&self) -> Result<QueryPlan, QueryError> {
        self.scope.plan_count(Dialect::active())
    }

    /// Executes this count query.
    pub async fn exec<S>(self, session: &mut S) -> Result<i64, DbError>
    where
        S: DbSession,
    {
        let stmt = statement_from_plan(self.plan()?, Arguments::default())?;
        session.fetch_scalar(stmt).await
    }
}

impl Exists {
    /// Renders SQL and parameter metadata without executing the query.
    pub fn plan(&self) -> Result<QueryPlan, QueryError> {
        self.scope.plan_exists(Dialect::active())
    }

    /// Executes this exists query.
    pub async fn exec<S>(self, session: &mut S) -> Result<bool, DbError>
    where
        S: DbSession,
    {
        let stmt = statement_from_plan(self.plan()?, Arguments::default())?;
        session.fetch_scalar(stmt).await
    }
}

impl<V> Scalar<V> {
    /// Renders SQL and parameter metadata without executing the query.
    pub fn plan(&self) -> Result<QueryPlan, QueryError> {
        self.scope.plan_scalar(self.expr.clone(), Dialect::active())
    }

    /// Executes this scalar query.
    pub async fn exec<S>(self, session: &mut S) -> Result<V, DbError>
    where
        V: for<'d> sqlx::Decode<'d, crate::commons::Database>
            + sqlx::Type<crate::commons::Database>
            + Send
            + Unpin
            + 'static,
        S: DbSession,
    {
        let stmt = statement_from_plan(self.plan()?, Arguments::default())?;
        session.fetch_scalar(stmt).await
    }
}
