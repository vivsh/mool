//! Scalar and aggregate executable implementations.

use crate::commons::Arguments;
use crate::executor::{DBSession, DbError};
use crate::placeholders::Dialect;

use super::super::binds::statement_from_plan;
use super::super::plan::QueryPlan;
use super::{Count, Exists, Scalar};
use crate::QueryError;

impl Count {
    /// Renders SQL and parameter metadata without executing the query.
    pub fn plan(&self, dialect: Dialect) -> Result<QueryPlan, QueryError> {
        self.scope.plan_count(dialect)
    }

    /// Executes this count query.
    pub async fn exec<S>(self, session: &mut S) -> Result<i64, DbError>
    where
        S: DBSession,
    {
        let stmt = statement_from_plan(self.plan(Dialect::active())?, Arguments::default())?;
        session.fetch_scalar(stmt).await
    }
}

impl Exists {
    /// Renders SQL and parameter metadata without executing the query.
    pub fn plan(&self, dialect: Dialect) -> Result<QueryPlan, QueryError> {
        self.scope.plan_exists(dialect)
    }

    /// Executes this exists query.
    pub async fn exec<S>(self, session: &mut S) -> Result<bool, DbError>
    where
        S: DBSession,
    {
        let stmt = statement_from_plan(self.plan(Dialect::active())?, Arguments::default())?;
        session.fetch_scalar(stmt).await
    }
}

impl<V> Scalar<V> {
    /// Renders SQL and parameter metadata without executing the query.
    pub fn plan(&self, dialect: Dialect) -> Result<QueryPlan, QueryError> {
        self.scope.plan_scalar(self.expr.clone(), dialect)
    }

    /// Executes this scalar query.
    pub async fn exec<S>(self, session: &mut S) -> Result<V, DbError>
    where
        V: for<'d> sqlx::Decode<'d, crate::commons::Database>
            + sqlx::Type<crate::commons::Database>
            + Send
            + Unpin
            + 'static,
        S: DBSession,
    {
        let stmt = statement_from_plan(self.plan(Dialect::active())?, Arguments::default())?;
        session.fetch_scalar(stmt).await
    }
}
