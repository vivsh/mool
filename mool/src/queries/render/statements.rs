//! Statement-level rendering for typed queries.

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
        self.render_projection(scope, model, &mut sql)?;
        self.render_from(model, &mut sql)?;
        self.render_filters(scope, RenderMode::Select(model), &mut sql)?;
        self.render_groups(scope, RenderMode::Select(model), &mut sql)?;
        self.render_having(scope, RenderMode::Select(model), &mut sql)?;
        self.render_orders(scope, RenderMode::Select(model), &mut sql)?;
        self.render_slice(slice, &mut sql);
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
        upsert: bool,
        conflict: &[ColumnRef],
        returning: Option<&SelectModel>,
    ) -> Result<String, QueryError> {
        if rows == 0 {
            return Err(QueryError::BindError(
                "cannot insert empty list".to_string(),
            ));
        }
        let mut sql = String::new();
        self.render_with(scope, &mut sql)?;
        sql.push_str("INSERT INTO ");
        self.render_insert_head(scope, columns, &mut sql)?;
        sql.push_str(" VALUES ");
        self.render_values_grid(rows, columns.len(), &mut sql);
        self.render_insert_tail(scope, columns, upsert, conflict, returning, &mut sql)?;
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

    pub(in crate::queries) fn render_count(
        &mut self,
        scope: &QueryScope,
        model: &SelectModel,
    ) -> Result<String, QueryError> {
        let mut sql = String::new();
        self.render_with(scope, &mut sql)?;
        sql.push_str("SELECT COUNT(*)");
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
}
