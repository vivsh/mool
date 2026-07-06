//! SELECT and RETURNING projection rendering.

use super::super::expr::ExprNode;
use super::super::scope::QueryScope;
use super::super::validate::output_column;
use super::{RenderMode, Renderer, SelectModel};
use crate::QueryError;

impl Renderer {
    pub(super) fn render_projection(
        &mut self,
        scope: &QueryScope,
        model: &SelectModel,
        sql: &mut String,
    ) -> Result<(), QueryError> {
        if model.columns.is_empty() {
            return Err(QueryError::InvalidProjection(
                "typed projection has no selected columns".to_string(),
            ));
        }
        for (idx, column) in model.columns.iter().enumerate() {
            if idx > 0 {
                sql.push_str(", ");
            }
            self.render_projection_column(scope, model, column, sql)?;
        }
        Ok(())
    }

    pub(super) fn render_returning(
        &mut self,
        scope: &QueryScope,
        model: Option<&SelectModel>,
        sql: &mut String,
    ) -> Result<(), QueryError> {
        let Some(model) = model else {
            return Ok(());
        };
        let mut columns = Vec::with_capacity(model.columns.len());
        for column in &model.columns {
            columns.push(self.render_returning_column(scope, model, column)?);
        }
        sql.push_str(&self.dialect_renderer.render_returning(&columns)?);
        Ok(())
    }

    fn render_projection_column(
        &mut self,
        scope: &QueryScope,
        model: &SelectModel,
        column: &str,
        sql: &mut String,
    ) -> Result<(), QueryError> {
        if let Some(expr) = select_expr(scope, column) {
            sql.push_str(&self.render_expr(expr, RenderMode::Select(model))?);
            sql.push_str(" AS ");
            sql.push_str(&output_column(column)?);
        } else if column == "*" {
            sql.push('*');
        } else {
            sql.push_str(&self.resolve_model_column(column, model)?);
        }
        Ok(())
    }

    fn render_returning_column(
        &mut self,
        scope: &QueryScope,
        _model: &SelectModel,
        column: &str,
    ) -> Result<String, QueryError> {
        let Some(expr) = select_expr(scope, column) else {
            return output_column(column);
        };
        Ok(format!(
            "{} AS {}",
            self.render_expr(
                expr,
                RenderMode::MutationRoot {
                    source: &scope.source
                }
            )?,
            output_column(column)?
        ))
    }
}

fn select_expr<'a>(scope: &'a QueryScope, column: &str) -> Option<&'a ExprNode> {
    let output = output_column(column).ok();
    scope
        .output_assignments
        .iter()
        .find(|assignment| {
            assignment.target.name.as_ref() == column
                || output.as_deref() == Some(assignment.target.name.as_ref())
        })
        .map(|assignment| &assignment.expr)
}
