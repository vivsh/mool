//! Write-value and upsert rendering.

use super::super::binds::upsert_update_columns;
use super::super::dialect::DialectFeature;
use super::super::expr::ColumnRef;
use super::super::scope::QueryScope;
use super::super::values::WriteSlot;
use super::{RenderMode, Renderer};
use crate::QueryError;

impl Renderer {
    pub(super) fn render_values_grid(&self, rows: usize, cols: usize, sql: &mut String) {
        for row in 0..rows {
            if row > 0 {
                sql.push_str(", ");
            }
            sql.push('(');
            self.render_value_row(row, cols, sql);
            sql.push(')');
        }
    }

    pub(super) fn render_write_slots(
        &mut self,
        scope: &QueryScope,
        slots: &[WriteSlot],
        sql: &mut String,
    ) -> Result<(), QueryError> {
        for (idx, slot) in slots.iter().enumerate() {
            if idx > 0 {
                sql.push_str(", ");
            }
            sql.push_str(&self.render_write_slot(scope, slot)?);
        }
        Ok(())
    }

    pub(super) fn render_write_slot(
        &mut self,
        scope: &QueryScope,
        slot: &WriteSlot,
    ) -> Result<String, QueryError> {
        match slot {
            WriteSlot::Prebound(position) => Ok(self.placeholder(*position)),
            WriteSlot::Expr(expr) => self.render_expr(
                expr,
                RenderMode::MutationRoot {
                    source: &scope.source,
                },
            ),
        }
    }

    pub(super) fn render_upsert(
        &self,
        columns: &[String],
        conflict: &[ColumnRef],
        sql: &mut String,
    ) -> Result<(), QueryError> {
        let update_columns = upsert_update_columns(columns, conflict)?;
        self.dialect_renderer
            .validate_feature(DialectFeature::Upsert)?;
        sql.push_str(
            &self
                .dialect_renderer
                .render_upsert(conflict, &update_columns)?,
        );
        Ok(())
    }

    fn render_value_row(&self, row: usize, cols: usize, sql: &mut String) {
        for col in 0..cols {
            if col > 0 {
                sql.push_str(", ");
            }
            let position = row * cols + col + 1;
            sql.push_str(&self.placeholder(position));
        }
    }
}
