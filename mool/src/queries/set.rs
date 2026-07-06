//! Set-operation executables for typed queries.

use std::collections::HashMap;
use std::marker::PhantomData;

use crate::argvalue::ArgValue;
use crate::commons::{Arguments, Row};
use crate::executor::{DBSession, DbError};
use crate::interfaces::Record;
use crate::placeholders::Dialect;

use super::binds::statement_from_plan;
use super::handles::{Var, VarId};
use super::plan::QueryPlan;
use super::scope::QueryScope;
use crate::QueryError;

/// SQL set operation used to combine two compatible read queries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetOp {
    /// `UNION`.
    Union,
    /// `UNION ALL`.
    UnionAll,
    /// `EXCEPT`.
    Except,
}

impl SetOp {
    pub(super) fn sql(self) -> &'static str {
        match self {
            Self::Union => "UNION",
            Self::UnionAll => "UNION ALL",
            Self::Except => "EXCEPT",
        }
    }
}

/// Executable set operation produced by `union`, `union_all`, or `except`.
#[derive(Clone)]
pub struct Set<T> {
    pub(super) left: QueryScope,
    pub(super) right: QueryScope,
    pub(super) op: SetOp,
    pub(super) binds: HashMap<VarId, ArgValue>,
    pub(super) errors: Vec<QueryError>,
    pub(super) _marker: PhantomData<fn() -> T>,
}

impl<T> Set<T>
where
    T: Record + 'static,
{
    pub(super) fn new(left: QueryScope, right: QueryScope, op: SetOp) -> Self {
        Self {
            left,
            right,
            op,
            binds: HashMap::new(),
            errors: Vec::new(),
            _marker: PhantomData,
        }
    }

    /// Binds a runtime value for a `var(...)` used by either side.
    pub fn bind<V>(mut self, var: &Var<V>, value: V) -> Self
    where
        V: Clone
            + for<'q> sqlx::Encode<'q, crate::commons::Database>
            + sqlx::Type<crate::commons::Database>
            + Send
            + Sync
            + 'static,
    {
        match self.binds.entry(var.data.id) {
            std::collections::hash_map::Entry::Occupied(_) => {
                let name = var.name().unwrap_or("anonymous var");
                self.errors.push(QueryError::BindError(format!(
                    "duplicate binding for '{}'",
                    name
                )));
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(ArgValue::new(value));
            }
        }
        self
    }

    /// Renders SQL and parameter metadata without executing the set query.
    pub fn plan(&self, dialect: Dialect) -> Result<QueryPlan, QueryError> {
        self.left
            .plan_set_all::<T>(&self.right, self.op, &self.binds, &self.errors, dialect)
    }

    /// Executes this set query against a database session.
    pub async fn exec<S>(self, session: &mut S) -> Result<Vec<T>, DbError>
    where
        T: for<'r> sqlx::FromRow<'r, Row> + Send + Unpin + 'static,
        S: DBSession,
    {
        let stmt = statement_from_plan(self.plan(Dialect::active())?, Arguments::default())?;
        session.fetch_all(stmt).await
    }
}
