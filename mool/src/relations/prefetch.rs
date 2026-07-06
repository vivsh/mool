//! Explicit prefetch support traits.

use std::collections::HashMap;
use std::hash::Hash;

use crate::interfaces::Record;

use super::backref::ManyBackref;

/// Extracts grouping keys used by explicit prefetch loading.
pub trait PrefetchKey<R: ManyBackref> {
    /// Join key type used to group children under parents.
    type Key: Eq + Hash + Clone;

    /// Key for the parent record being hydrated.
    fn parent_key(&self) -> Self::Key;

    /// Key for a related child row.
    fn child_key(child: &R::To) -> Self::Key;
}

/// A record or model that can receive an explicit prefetch result.
pub trait ReceivesPrefetch<R: ManyBackref>: PrefetchKey<R> {
    /// Inserts prefetched child rows into the receiver.
    fn receive_prefetch(&mut self, rows: Vec<R::To>);
}

/// Starts an explicit many-relation prefetch executable.
pub fn prefetch<R, P>(parents: Vec<P>) -> Prefetch<R, P>
where
    R: ManyBackref,
{
    Prefetch {
        parents,
        _marker: std::marker::PhantomData,
    }
}

/// Explicit one-SQL prefetch executable for many backrefs.
pub struct Prefetch<R, P>
where
    R: ManyBackref,
{
    parents: Vec<P>,
    _marker: std::marker::PhantomData<fn() -> R>,
}

impl<R, P> Prefetch<R, P>
where
    R: ManyBackref,
    R::To: for<'r> sqlx::FromRow<'r, crate::Row> + Send + Unpin + 'static,
    P: ReceivesPrefetch<R>,
    P::Key: Clone
        + for<'q> sqlx::Encode<'q, crate::Database>
        + sqlx::Type<crate::Database>
        + Send
        + 'static,
{
    /// Executes the prefetch and returns the hydrated parent records.
    pub async fn exec<S>(mut self, session: &mut S) -> Result<Vec<P>, crate::DbError>
    where
        S: crate::executor::DBSession,
    {
        if self.parents.is_empty() {
            return Ok(self.parents);
        }
        let mut children = self.fetch_children(session).await?;
        for parent in &mut self.parents {
            let key = parent.parent_key();
            let rows = children.remove(&key).unwrap_or_default();
            parent.receive_prefetch(rows);
        }
        Ok(self.parents)
    }

    async fn fetch_children<S>(
        &self,
        session: &mut S,
    ) -> Result<HashMap<P::Key, Vec<R::To>>, crate::DbError>
    where
        S: crate::executor::DBSession,
    {
        let stmt = self.statement()?;
        let rows: Vec<R::To> = session.fetch_all(stmt).await?;
        let mut grouped = HashMap::with_capacity(rows.len());
        for row in rows {
            grouped
                .entry(P::child_key(&row))
                .or_insert_with(Vec::new)
                .push(row);
        }
        Ok(grouped)
    }

    fn statement(&self) -> Result<crate::Statement, crate::DbError> {
        let meta = R::meta();
        let Some(column) = meta.columns.first() else {
            return Err(crate::DbError::Unsupported("prefetch without join columns"));
        };
        if meta.columns.len() != 1 {
            return Err(crate::DbError::Unsupported("composite prefetch"));
        }
        validate_identifier(meta.table_name)?;
        validate_identifier(column.to)?;
        let sql = self.prefetch_sql(meta.table_schema, meta.table_name, column.to)?;
        let mut stmt = crate::Statement::from_str(&sql);
        for parent in &self.parents {
            stmt = stmt.bind(parent.parent_key());
        }
        Ok(stmt)
    }

    fn prefetch_sql(
        &self,
        schema: Option<&str>,
        table: &str,
        column: &str,
    ) -> Result<String, crate::DbError> {
        if let Some(schema) = schema {
            validate_identifier(schema)?;
        }
        let mut sql = String::from("SELECT ");
        sql.push_str(&R::To::record_column_names().join(", "));
        sql.push_str(" FROM ");
        if let Some(schema) = schema {
            sql.push_str(schema);
            sql.push('.');
        }
        sql.push_str(table);
        sql.push_str(" WHERE ");
        sql.push_str(column);
        sql.push_str(" IN (");
        push_placeholders(self.parents.len(), &mut sql);
        sql.push(')');
        Ok(sql)
    }
}

fn push_placeholders(count: usize, sql: &mut String) {
    for idx in 0..count {
        if idx > 0 {
            sql.push_str(", ");
        }
        match crate::placeholders::Dialect::active() {
            crate::placeholders::Dialect::Postgres => {
                sql.push('$');
                sql.push_str(&(idx + 1).to_string());
            }
            crate::placeholders::Dialect::Mysql | crate::placeholders::Dialect::Sqlite => {
                sql.push('?')
            }
        }
    }
}

fn validate_identifier(value: &str) -> Result<(), crate::DbError> {
    let Some(first) = value.chars().next() else {
        return Err(crate::DbError::QuerySet(
            crate::QueryError::InvalidIdentifier(value.to_string()),
        ));
    };
    if (first.is_ascii_alphabetic() || first == '_')
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        return Ok(());
    }
    Err(crate::DbError::QuerySet(
        crate::QueryError::InvalidIdentifier(value.to_string()),
    ))
}
