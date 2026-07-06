//! Owned write executable implementations.

use crate::executor::{DBSession, DbError};
use crate::interfaces::Record;
use crate::placeholders::Dialect;

use super::super::plan::QueryPlan;
use super::{
    BatchInsert, BatchUpsert, Insert, OwnedBatchInsert, OwnedBatchUpsert, OwnedInsert, OwnedUpdate,
    Update,
};
use crate::QueryError;

impl<T> OwnedInsert<T>
where
    T: Record,
{
    /// Renders SQL and parameter metadata without executing the insert.
    pub fn plan(&self, dialect: Dialect) -> Result<QueryPlan, QueryError> {
        self.scope.plan_insert(&&self.row, dialect)
    }

    /// Executes this owned insert.
    pub async fn exec<S>(self, session: &mut S) -> Result<u64, DbError>
    where
        S: DBSession,
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
    pub fn plan(&self, dialect: Dialect) -> Result<QueryPlan, QueryError> {
        self.scope.plan_update(&&self.row, dialect)
    }

    /// Executes this owned update.
    pub async fn exec<S>(self, session: &mut S) -> Result<u64, DbError>
    where
        S: DBSession,
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
    /// Renders SQL and parameter metadata without executing the insert.
    pub fn plan(&self, dialect: Dialect) -> Result<QueryPlan, QueryError> {
        self.scope.plan_batch_insert(&self.rows, dialect)
    }

    /// Executes this owned batch insert.
    pub async fn exec<S>(self, session: &mut S) -> Result<u64, DbError>
    where
        S: DBSession,
    {
        BatchInsert {
            scope: self.scope,
            rows: &self.rows,
        }
        .exec(session)
        .await
    }
}

impl<T> OwnedBatchUpsert<T>
where
    T: Record + 'static,
{
    /// Renders SQL and parameter metadata without executing the upsert.
    pub fn plan(&self, dialect: Dialect) -> Result<QueryPlan, QueryError> {
        self.scope
            .plan_batch_upsert(&self.rows, self.conflict.clone(), dialect)
    }

    /// Executes this owned batch upsert.
    pub async fn exec<S>(self, session: &mut S) -> Result<u64, DbError>
    where
        S: DBSession,
    {
        BatchUpsert {
            scope: self.scope,
            rows: &self.rows,
            conflict: self.conflict,
        }
        .exec(session)
        .await
    }
}
