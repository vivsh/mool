//! Explicit prefetch support traits.

use std::collections::{HashMap, HashSet};
use std::hash::Hash;

use crate::interfaces::Record;

use super::backref::ManyBackref;

/// Extracts grouping keys used by explicit prefetch loading.
pub trait PrefetchKey<R: ManyBackref> {
    /// Join key type used to group children under parents.
    type Key: Eq + Hash + Clone;

    /// Number of join-key values bound for each parent.
    const KEY_ARITY: usize;

    /// Key for the parent record being hydrated.
    fn parent_key(&self) -> Self::Key;

    /// Key for a related child row.
    fn child_key(child: &R::To) -> Self::Key;

    /// Appends this parent's join-key values in relation-column order.
    fn bind_parent_key(&self, statement: crate::Statement) -> crate::Statement;
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
    R::To: for<'r> sqlx::FromRow<'r, crate::backend::Row> + Send + Unpin + 'static,
    P: ReceivesPrefetch<R>,
    P::Key: Clone,
{
    /// Executes the prefetch and returns the hydrated parent records.
    pub async fn exec<S>(mut self, session: &mut S) -> Result<Vec<P>, crate::DbError>
    where
        S: crate::executor::DbSession,
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
        S: crate::executor::DbSession,
    {
        self.validate_relation()?;
        let indexes = self.unique_parent_indexes();
        let rows_per_statement = crate::backend::PARAMETER_LIMIT
            .checked_div(P::KEY_ARITY)
            .filter(|rows| *rows > 0)
            .ok_or_else(|| crate::DbError::Relation {
                relation: R::NAME,
                reason: "prefetch key exceeds the backend parameter limit".to_string(),
            })?;
        let mut grouped: HashMap<P::Key, Vec<R::To>> = HashMap::new();
        for indexes in indexes.chunks(rows_per_statement) {
            let stmt = self.statement(indexes)?;
            let rows: Vec<R::To> = session.fetch_all(stmt).await?;
            for row in rows {
                grouped.entry(P::child_key(&row)).or_default().push(row);
            }
        }
        Ok(grouped)
    }

    fn statement(&self, parent_indexes: &[usize]) -> Result<crate::Statement, crate::DbError> {
        let meta = R::meta();
        let columns = meta
            .columns
            .iter()
            .map(|column| column.to)
            .collect::<Vec<_>>();
        let sql = self.prefetch_sql(
            meta.table_schema,
            meta.table_name,
            &columns,
            parent_indexes.len(),
        )?;
        let mut stmt = crate::Statement::raw(&sql);
        for index in parent_indexes {
            stmt = self.parents[*index].bind_parent_key(stmt);
        }
        Ok(stmt)
    }

    fn validate_relation(&self) -> Result<(), crate::DbError> {
        let meta = R::meta();
        if meta.columns.is_empty() {
            return Err(crate::DbError::Relation {
                relation: R::NAME,
                reason: "prefetch requires at least one join column".to_string(),
            });
        }
        if meta.columns.len() != P::KEY_ARITY {
            return Err(crate::DbError::Relation {
                relation: R::NAME,
                reason: format!(
                    "relation defines {} columns but PrefetchKey declares {} values",
                    meta.columns.len(),
                    P::KEY_ARITY
                ),
            });
        }
        validate_identifier(meta.table_name)?;
        if let Some(schema) = meta.table_schema {
            validate_identifier(schema)?;
        }
        for column in meta.columns {
            validate_identifier(column.from)?;
            validate_identifier(column.to)?;
        }
        for column in R::To::record_column_names() {
            validate_qualified_identifier(&column)?;
        }
        Ok(())
    }

    fn unique_parent_indexes(&self) -> Vec<usize> {
        let mut keys = HashSet::with_capacity(self.parents.len());
        self.parents
            .iter()
            .enumerate()
            .filter_map(|(index, parent)| keys.insert(parent.parent_key()).then_some(index))
            .collect()
    }

    fn prefetch_sql(
        &self,
        schema: Option<&str>,
        table: &str,
        columns: &[&str],
        rows: usize,
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
        push_prefetch_lhs(columns, &mut sql);
        sql.push_str(" IN (");
        push_key_rows(rows, columns.len(), &mut sql);
        sql.push(')');
        Ok(sql)
    }
}

fn validate_qualified_identifier(value: &str) -> Result<(), crate::DbError> {
    for part in value.split('.') {
        validate_identifier(part)?;
    }
    Ok(())
}

fn push_prefetch_lhs(columns: &[&str], sql: &mut String) {
    if columns.len() > 1 {
        sql.push('(');
    }
    sql.push_str(&columns.join(", "));
    if columns.len() > 1 {
        sql.push(')');
    }
}

fn push_key_rows(rows: usize, columns: usize, sql: &mut String) {
    let mut position = 1;
    for row in 0..rows {
        if row > 0 {
            sql.push_str(", ");
        }
        if columns > 1 {
            sql.push('(');
        }
        for column in 0..columns {
            if column > 0 {
                sql.push_str(", ");
            }
            push_placeholder(position, sql);
            position += 1;
        }
        if columns > 1 {
            sql.push(')');
        }
    }
}

fn push_placeholder(position: usize, sql: &mut String) {
    match crate::placeholders::Dialect::active() {
        crate::placeholders::Dialect::Postgres => {
            sql.push('$');
            sql.push_str(&position.to_string());
        }
        crate::placeholders::Dialect::Mysql
        | crate::placeholders::Dialect::Mariadb
        | crate::placeholders::Dialect::Sqlite => sql.push('?'),
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
