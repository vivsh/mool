use std::collections::{HashMap, HashSet};

use super::{QueryError, Statement};
use crate::argvalue::ArgValue;
use crate::commons::{Arguments, Row};
use crate::executor::{DbError, DbSession};
use crate::placeholders::{
    Dialect, PlaceholderIter, PlaceholderPart, has_named_placeholder, resolve_placeholders,
};

/// Builder for raw SQL with Mool named-bind support.
pub struct RawQuery {
    sql: String,
    args: Arguments<'static>,
    named_args: HashMap<String, ArgValue>,
    error: Option<QueryError>,
}

impl RawQuery {
    pub(crate) fn new(sql: &str) -> Self {
        Self {
            sql: sql.to_string(),
            args: Arguments::default(),
            named_args: HashMap::new(),
            error: None,
        }
    }

    /// Bind a named argument used by `:name` placeholders.
    pub fn bind<T>(mut self, name: &str, val: T) -> Self
    where
        T: Clone
            + for<'q> sqlx::Encode<'q, crate::commons::Database>
            + sqlx::Type<crate::commons::Database>
            + Send
            + Sync
            + 'static,
    {
        if self.error.is_some() {
            return self;
        }
        self.named_args.insert(name.to_string(), ArgValue::new(val));
        self
    }

    /// Renders this raw query into a statement for the selected backend.
    pub fn to_statement(mut self) -> Result<Statement, QueryError> {
        let dialect = Dialect::active();
        if let Some(err) = self.error {
            return Err(err);
        }
        if self.named_args.is_empty() && !has_named_placeholder(&self.sql) {
            return Ok(Statement::new(&self.sql, self.args));
        }
        self.validate_used_binds()?;
        let final_sql = resolve_placeholders(&self.sql, &mut self.args, &self.named_args, dialect)?;
        Ok(Statement::new(&final_sql, self.args))
    }

    fn validate_used_binds(&self) -> Result<(), QueryError> {
        if self.named_args.is_empty() {
            return Ok(());
        }
        let used = named_placeholders(&self.sql);
        if let Some(name) = self.named_args.keys().find(|name| !used.contains(*name)) {
            return Err(QueryError::UnusedBinding(name.clone()));
        }
        Ok(())
    }

    fn into_statement(self) -> Result<Statement, QueryError> {
        self.to_statement()
    }

    pub async fn execute(self, session: &mut impl DbSession) -> Result<u64, DbError> {
        let stmt = self.into_statement()?;
        session.execute(stmt).await
    }

    pub async fn one<M>(self, session: &mut impl DbSession) -> Result<M, DbError>
    where
        M: for<'r> sqlx::FromRow<'r, Row> + Send + Unpin + 'static,
    {
        let stmt = self.into_statement()?;
        session.fetch_one(stmt).await
    }

    pub async fn all<M>(self, session: &mut impl DbSession) -> Result<Vec<M>, DbError>
    where
        M: for<'r> sqlx::FromRow<'r, Row> + Send + Unpin + 'static,
    {
        let stmt = self.into_statement()?;
        session.fetch_all(stmt).await
    }

    pub async fn first<M>(self, session: &mut impl DbSession) -> Result<Option<M>, DbError>
    where
        M: for<'r> sqlx::FromRow<'r, Row> + Send + Unpin + 'static,
    {
        let stmt = self.into_statement()?;
        session.fetch_optional(stmt).await
    }

    pub async fn scalar<T>(self, session: &mut impl DbSession) -> Result<T, DbError>
    where
        for<'r> (T,): sqlx::FromRow<'r, Row>,
        T: Send + Unpin + 'static,
    {
        let stmt = self.into_statement()?;
        let row: (T,) = session.fetch_one(stmt).await?;
        Ok(row.0)
    }
}

fn named_placeholders(sql: &str) -> HashSet<String> {
    PlaceholderIter::new(sql)
        .filter_map(|part| match part {
            PlaceholderPart::Placeholder(name) => Some(name.to_string()),
            PlaceholderPart::Sql(_) => None,
        })
        .collect()
}
