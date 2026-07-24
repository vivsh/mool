//! Statement-level rendering for typed queries.

use super::super::batch::BatchInsertMode;
use super::super::expr::{ColumnRef, ExprNode};
use super::super::scope::QueryScope;
use super::super::validate::{source_table, validate_identifier};
use super::super::values::WriteParts;
use super::{RenderMode, Renderer, SelectModel};
use crate::QueryError;

impl Renderer {
    pub(in crate::queries) fn render_select(
        &mut self,
        scope: &QueryScope,
        model: &SelectModel,
        slice: Option<(usize, usize)>,
    ) -> Result<String, QueryError> {
        let mut sql = String::new();
        self.render_with(scope, &mut sql)?;
        sql.push_str("SELECT ");
        if !scope.distinct_on.is_empty() {
            sql.push_str("DISTINCT ON (");
            for (index, expr) in scope.distinct_on.iter().enumerate() {
                if index > 0 {
                    sql.push_str(", ");
                }
                sql.push_str(&self.render_expr(expr, RenderMode::Select(model))?);
            }
            sql.push_str(") ");
        } else if scope.distinct {
            sql.push_str("DISTINCT ");
        }
        self.render_projection(scope, model, &mut sql)?;
        self.render_from(model, &mut sql)?;
        self.render_filters(scope, RenderMode::Select(model), &mut sql)?;
        self.render_groups(scope, RenderMode::Select(model), &mut sql)?;
        self.render_having(scope, RenderMode::Select(model), &mut sql)?;
        self.render_orders(scope, RenderMode::Select(model), &mut sql)?;
        self.render_slice(slice, &mut sql);
        #[cfg(any(feature = "postgres", feature = "mysql", feature = "mariadb"))]
        self.render_lock(scope, &mut sql);
        Ok(sql)
    }

    pub(in crate::queries) fn render_insert(
        &mut self,
        scope: &QueryScope,
        parts: &WriteParts,
        upsert: bool,
        conflict: &[ColumnRef],
        returning: Option<&SelectModel>,
    ) -> Result<String, QueryError> {
        let mut sql = String::new();
        self.render_with(scope, &mut sql)?;
        sql.push_str("INSERT INTO ");
        self.render_insert_head(scope, &parts.columns, &mut sql)?;
        sql.push_str(" VALUES (");
        self.render_write_slots(scope, &parts.slots, &mut sql)?;
        sql.push(')');
        self.render_insert_tail(scope, &parts.columns, upsert, conflict, returning, &mut sql)?;
        Ok(sql)
    }

    pub(in crate::queries) fn render_batch_insert(
        &mut self,
        scope: &QueryScope,
        columns: &[String],
        rows: usize,
        mode: &BatchInsertMode,
        returning: Option<&SelectModel>,
    ) -> Result<String, QueryError> {
        if rows == 0 {
            return Err(QueryError::BindError(
                "cannot insert empty list".to_string(),
            ));
        }
        let mut sql = String::new();
        self.render_with(scope, &mut sql)?;
        sql.push_str(match mode {
            #[cfg(any(feature = "mysql", feature = "mariadb"))]
            BatchInsertMode::IgnoreErrors => "INSERT IGNORE INTO ",
            _ => "INSERT INTO ",
        });
        self.render_insert_head(scope, columns, &mut sql)?;
        sql.push_str(" VALUES ");
        self.render_values_grid(rows, columns.len(), &mut sql);
        self.render_batch_insert_tail(scope, columns, mode, returning, &mut sql)?;
        Ok(sql)
    }

    #[cfg(feature = "postgres")]
    pub(in crate::queries) fn render_batch_unnest(
        &mut self,
        scope: &QueryScope,
        columns: &[String],
        mode: &BatchInsertMode,
        returning: Option<&SelectModel>,
    ) -> Result<String, QueryError> {
        if columns.is_empty() {
            return Err(QueryError::BindError(
                "UNNEST requires at least one writable column".to_string(),
            ));
        }
        let mut sql = String::from("INSERT INTO ");
        self.render_insert_head(scope, columns, &mut sql)?;
        sql.push_str(" SELECT ");
        for (index, column) in columns.iter().enumerate() {
            if index > 0 {
                sql.push_str(", ");
            }
            sql.push_str("__mool_input.");
            sql.push_str(column);
        }
        sql.push_str(" FROM UNNEST(");
        for index in 0..columns.len() {
            if index > 0 {
                sql.push_str(", ");
            }
            sql.push_str(&self.placeholder(index + 1));
        }
        sql.push_str(") AS __mool_input (");
        sql.push_str(&columns.join(", "));
        sql.push(')');
        self.render_batch_insert_tail(scope, columns, mode, returning, &mut sql)?;
        Ok(sql)
    }

    pub(in crate::queries) fn render_update(
        &mut self,
        scope: &QueryScope,
        parts: &WriteParts,
        returning: Option<&SelectModel>,
    ) -> Result<String, QueryError> {
        let mut sql = String::new();
        self.render_with(scope, &mut sql)?;
        sql.push_str("UPDATE ");
        let table = source_table(&scope.source)?;
        sql.push_str(&self.render_table_name(table)?);
        sql.push_str(" SET ");
        self.render_update_set(scope, parts, &mut sql)?;
        self.render_filters(
            scope,
            RenderMode::MutationRoot {
                source: &scope.source,
            },
            &mut sql,
        )?;
        self.render_returning(scope, returning, &mut sql)?;
        Ok(sql)
    }

    pub(in crate::queries) fn render_delete(
        &mut self,
        scope: &QueryScope,
        returning: Option<&SelectModel>,
    ) -> Result<String, QueryError> {
        let mut sql = String::new();
        self.render_with(scope, &mut sql)?;
        sql.push_str("DELETE FROM ");
        let table = source_table(&scope.source)?;
        sql.push_str(&self.render_table_name(table)?);
        self.render_filters(
            scope,
            RenderMode::MutationRoot {
                source: &scope.source,
            },
            &mut sql,
        )?;
        self.render_returning(scope, returning, &mut sql)?;
        Ok(sql)
    }

    pub(in crate::queries) fn render_batch_update(
        &mut self,
        scope: &QueryScope,
        primary_keys: &[&str],
        update_columns: &[&str],
        rows: usize,
        returning: Option<&SelectModel>,
    ) -> Result<String, QueryError> {
        if rows == 0 {
            return Err(QueryError::EmptyBatch {
                operation: "batch update",
            });
        }
        let input_columns = primary_keys
            .iter()
            .chain(update_columns)
            .map(|column| format!("__mool_{column}"))
            .collect::<Vec<_>>();
        for column in primary_keys.iter().chain(update_columns) {
            validate_identifier(column)?;
        }
        let sql = match self.dialect {
            crate::placeholders::Dialect::Postgres => self.render_postgres_batch_update(
                scope,
                primary_keys,
                update_columns,
                &input_columns,
                rows,
            )?,
            crate::placeholders::Dialect::Sqlite => self.render_sqlite_batch_update(
                scope,
                primary_keys,
                update_columns,
                &input_columns,
                rows,
            )?,
            crate::placeholders::Dialect::Mysql | crate::placeholders::Dialect::Mariadb => self
                .render_mysql_family_batch_update(
                    scope,
                    primary_keys,
                    update_columns,
                    &input_columns,
                    rows,
                )?,
        };
        let mut sql = sql;
        self.render_returning(scope, returning, &mut sql)?;
        Ok(sql)
    }

    fn render_postgres_batch_update(
        &mut self,
        scope: &QueryScope,
        primary_keys: &[&str],
        update_columns: &[&str],
        input_columns: &[String],
        rows: usize,
    ) -> Result<String, QueryError> {
        let mut sql = self.batch_update_head(scope, update_columns)?;
        sql.push_str(" FROM (VALUES ");
        self.render_values_grid(rows, input_columns.len(), &mut sql);
        sql.push_str(") AS __mool_input (");
        sql.push_str(&input_columns.join(", "));
        sql.push(')');
        self.append_batch_update_predicates(scope, primary_keys, &mut sql)?;
        Ok(sql)
    }

    fn render_sqlite_batch_update(
        &mut self,
        scope: &QueryScope,
        primary_keys: &[&str],
        update_columns: &[&str],
        input_columns: &[String],
        rows: usize,
    ) -> Result<String, QueryError> {
        let mut sql = String::from("WITH __mool_input (");
        sql.push_str(&input_columns.join(", "));
        sql.push_str(") AS (VALUES ");
        self.render_values_grid(rows, input_columns.len(), &mut sql);
        sql.push_str(") ");
        sql.push_str(&self.batch_update_head(scope, update_columns)?);
        sql.push_str(" FROM __mool_input");
        self.append_batch_update_predicates(scope, primary_keys, &mut sql)?;
        Ok(sql)
    }

    fn render_mysql_family_batch_update(
        &mut self,
        scope: &QueryScope,
        primary_keys: &[&str],
        update_columns: &[&str],
        input_columns: &[String],
        rows: usize,
    ) -> Result<String, QueryError> {
        let table = source_table(&scope.source)?;
        let mut sql = String::from("UPDATE ");
        sql.push_str(&self.render_table_name(table)?);
        sql.push_str(" AS __mool_target JOIN (");
        for row in 0..rows {
            if row > 0 {
                sql.push_str(" UNION ALL ");
            }
            sql.push_str("SELECT ");
            for (column, name) in input_columns.iter().enumerate() {
                if column > 0 {
                    sql.push_str(", ");
                }
                sql.push_str(&self.placeholder(row * input_columns.len() + column + 1));
                if row == 0 {
                    sql.push_str(" AS ");
                    sql.push_str(name);
                }
            }
        }
        sql.push_str(") AS __mool_input ON ");
        self.render_batch_update_key_match(primary_keys, &mut sql);
        sql.push_str(" SET ");
        self.render_batch_update_assignments(update_columns, &mut sql);
        self.append_scope_filters(scope, &mut sql)?;
        Ok(sql)
    }

    fn batch_update_head(
        &self,
        scope: &QueryScope,
        update_columns: &[&str],
    ) -> Result<String, QueryError> {
        let table = source_table(&scope.source)?;
        let mut sql = String::from("UPDATE ");
        sql.push_str(&self.render_table_name(table)?);
        sql.push_str(" AS __mool_target SET ");
        self.render_batch_update_assignments(update_columns, &mut sql);
        Ok(sql)
    }

    fn render_batch_update_assignments(&self, columns: &[&str], sql: &mut String) {
        for (index, column) in columns.iter().enumerate() {
            if index > 0 {
                sql.push_str(", ");
            }
            sql.push_str(column);
            sql.push_str(" = __mool_input.__mool_");
            sql.push_str(column);
        }
    }

    fn append_batch_update_predicates(
        &mut self,
        scope: &QueryScope,
        primary_keys: &[&str],
        sql: &mut String,
    ) -> Result<(), QueryError> {
        sql.push_str(" WHERE ");
        self.render_batch_update_key_match(primary_keys, sql);
        self.append_scope_filters_with_and(scope, sql)
    }

    fn render_batch_update_key_match(&self, primary_keys: &[&str], sql: &mut String) {
        for (index, column) in primary_keys.iter().enumerate() {
            if index > 0 {
                sql.push_str(" AND ");
            }
            sql.push_str("__mool_target.");
            sql.push_str(column);
            sql.push_str(" = __mool_input.__mool_");
            sql.push_str(column);
        }
    }

    fn append_scope_filters(
        &mut self,
        scope: &QueryScope,
        sql: &mut String,
    ) -> Result<(), QueryError> {
        self.render_filters(
            scope,
            RenderMode::MutationRoot {
                source: &scope.source,
            },
            sql,
        )
    }

    fn append_scope_filters_with_and(
        &mut self,
        scope: &QueryScope,
        sql: &mut String,
    ) -> Result<(), QueryError> {
        let mut filters = String::new();
        self.append_scope_filters(scope, &mut filters)?;
        if let Some(filters) = filters.strip_prefix(" WHERE ") {
            sql.push_str(" AND ");
            sql.push_str(filters);
        }
        Ok(())
    }

    pub(in crate::queries) fn render_count(
        &mut self,
        scope: &QueryScope,
        model: &SelectModel,
    ) -> Result<String, QueryError> {
        let mut sql = String::new();
        self.render_with(scope, &mut sql)?;
        if !scope.groups.is_empty() || !scope.having.is_empty() {
            sql.push_str("SELECT COUNT(*) FROM (");
            sql.push_str(&self.render_grouped_count_source(scope, model)?);
            sql.push_str(") __mool_count");
            return Ok(sql);
        }
        sql.push_str("SELECT COUNT(*)");
        self.render_from(model, &mut sql)?;
        self.render_filters(scope, RenderMode::Select(model), &mut sql)?;
        Ok(sql)
    }

    fn render_grouped_count_source(
        &mut self,
        scope: &QueryScope,
        model: &SelectModel,
    ) -> Result<String, QueryError> {
        let mut sql = String::from("SELECT 1");
        self.render_from(model, &mut sql)?;
        self.render_filters(scope, RenderMode::Select(model), &mut sql)?;
        self.render_groups(scope, RenderMode::Select(model), &mut sql)?;
        self.render_having(scope, RenderMode::Select(model), &mut sql)?;
        Ok(sql)
    }

    pub(in crate::queries) fn render_exists(
        &mut self,
        scope: &QueryScope,
        model: &SelectModel,
    ) -> Result<String, QueryError> {
        let mut sql = String::new();
        self.render_with(scope, &mut sql)?;
        sql.push_str("SELECT EXISTS(SELECT 1");
        self.render_from(model, &mut sql)?;
        self.render_filters(scope, RenderMode::Select(model), &mut sql)?;
        self.render_groups(scope, RenderMode::Select(model), &mut sql)?;
        self.render_having(scope, RenderMode::Select(model), &mut sql)?;
        sql.push(')');
        Ok(sql)
    }

    pub(in crate::queries) fn render_scalar(
        &mut self,
        scope: &QueryScope,
        model: &SelectModel,
        expr: &ExprNode,
    ) -> Result<String, QueryError> {
        let mut sql = String::new();
        self.render_with(scope, &mut sql)?;
        sql.push_str("SELECT ");
        let rendered = self.render_expr(expr, RenderMode::Select(model))?;
        sql.push_str(&rendered);
        self.render_from(model, &mut sql)?;
        self.render_filters(scope, RenderMode::Select(model), &mut sql)?;
        self.render_groups(scope, RenderMode::Select(model), &mut sql)?;
        self.render_having(scope, RenderMode::Select(model), &mut sql)?;
        self.render_orders(scope, RenderMode::Select(model), &mut sql)?;
        sql.push_str(" LIMIT 1");
        Ok(sql)
    }

    fn render_filters(
        &mut self,
        scope: &QueryScope,
        mode: RenderMode<'_>,
        sql: &mut String,
    ) -> Result<(), QueryError> {
        if scope.filters.is_empty() {
            return Ok(());
        }
        sql.push_str(" WHERE ");
        for (idx, filter) in scope.filters.iter().enumerate() {
            if idx > 0 {
                sql.push_str(" AND ");
            }
            sql.push_str(&self.render_expr(&filter.node, mode)?);
        }
        Ok(())
    }

    fn render_groups(
        &mut self,
        scope: &QueryScope,
        mode: RenderMode<'_>,
        sql: &mut String,
    ) -> Result<(), QueryError> {
        if scope.groups.is_empty() {
            return Ok(());
        }
        sql.push_str(" GROUP BY ");
        for (idx, group) in scope.groups.iter().enumerate() {
            if idx > 0 {
                sql.push_str(", ");
            }
            sql.push_str(&self.render_expr(group, mode)?);
        }
        Ok(())
    }

    fn render_having(
        &mut self,
        scope: &QueryScope,
        mode: RenderMode<'_>,
        sql: &mut String,
    ) -> Result<(), QueryError> {
        if scope.having.is_empty() {
            return Ok(());
        }
        sql.push_str(" HAVING ");
        for (idx, predicate) in scope.having.iter().enumerate() {
            if idx > 0 {
                sql.push_str(" AND ");
            }
            sql.push_str(&self.render_expr(&predicate.node, mode)?);
        }
        Ok(())
    }

    fn render_orders(
        &mut self,
        scope: &QueryScope,
        mode: RenderMode<'_>,
        sql: &mut String,
    ) -> Result<(), QueryError> {
        if scope.orders.is_empty() {
            return Ok(());
        }
        sql.push_str(" ORDER BY ");
        for (idx, order) in scope.orders.iter().enumerate() {
            if idx > 0 {
                sql.push_str(", ");
            }
            sql.push_str(&self.render_expr(&order.expr, mode)?);
            sql.push_str(if order.desc { " DESC" } else { " ASC" });
        }
        Ok(())
    }

    fn render_insert_head(
        &self,
        scope: &QueryScope,
        columns: &[String],
        sql: &mut String,
    ) -> Result<(), QueryError> {
        let table = source_table(&scope.source)?;
        sql.push_str(&self.render_table_name(table)?);
        sql.push_str(" (");
        sql.push_str(&columns.join(", "));
        sql.push(')');
        Ok(())
    }

    fn render_insert_tail(
        &mut self,
        scope: &QueryScope,
        columns: &[String],
        upsert: bool,
        conflict: &[ColumnRef],
        returning: Option<&SelectModel>,
        sql: &mut String,
    ) -> Result<(), QueryError> {
        if upsert {
            self.render_upsert(columns, conflict, sql)?;
        }
        self.render_returning(scope, returning, sql)
    }

    fn render_batch_insert_tail(
        &mut self,
        scope: &QueryScope,
        columns: &[String],
        mode: &BatchInsertMode,
        returning: Option<&SelectModel>,
        sql: &mut String,
    ) -> Result<(), QueryError> {
        match mode {
            BatchInsertMode::Insert => {}
            #[cfg(any(feature = "mysql", feature = "mariadb"))]
            BatchInsertMode::IgnoreErrors => {}
            #[cfg(any(feature = "postgres", feature = "sqlite"))]
            BatchInsertMode::Ignore(conflict) => sql.push_str(
                &self
                    .dialect_renderer
                    .render_ignore_conflicts(conflict.as_deref().unwrap_or_default())?,
            ),
            BatchInsertMode::Upsert {
                conflict,
                update_columns,
            } => self.render_upsert_selected(columns, conflict, update_columns.as_deref(), sql)?,
        }
        self.render_returning(scope, returning, sql)
    }

    fn render_update_set(
        &mut self,
        scope: &QueryScope,
        parts: &WriteParts,
        sql: &mut String,
    ) -> Result<(), QueryError> {
        for (idx, column) in parts.columns.iter().enumerate() {
            if idx > 0 {
                sql.push_str(", ");
            }
            validate_identifier(column)?;
            sql.push_str(column);
            sql.push_str(" = ");
            let Some(slot) = parts.slots.get(idx) else {
                return Err(QueryError::BindError("missing write value".to_string()));
            };
            sql.push_str(&self.render_write_slot(scope, slot)?);
        }
        Ok(())
    }

    fn render_slice(&self, slice: Option<(usize, usize)>, sql: &mut String) {
        if let Some((offset, count)) = slice {
            sql.push_str(" LIMIT ");
            sql.push_str(&count.to_string());
            sql.push_str(" OFFSET ");
            sql.push_str(&offset.to_string());
        }
    }

    #[cfg(any(feature = "postgres", feature = "mysql", feature = "mariadb"))]
    fn render_lock(&self, scope: &QueryScope, sql: &mut String) {
        let Some(lock) = scope.lock else {
            return;
        };
        let lock_sql = match (self.dialect(), lock) {
            (crate::placeholders::Dialect::Mariadb, crate::LockMode::Share) => {
                " LOCK IN SHARE MODE"
            }
            (_, crate::LockMode::Update) => " FOR UPDATE",
            (_, crate::LockMode::Share) => " FOR SHARE",
        };
        sql.push_str(lock_sql);
        #[cfg(any(feature = "postgres", feature = "mysql"))]
        if let Some(wait) = scope.lock_wait {
            sql.push_str(match wait {
                crate::query_error::LockWait::NoWait => " NOWAIT",
                crate::query_error::LockWait::SkipLocked => " SKIP LOCKED",
            });
        }
    }
}
