//! SQL expression rendering.

use super::super::dialect::DialectFeature;
use super::super::expr::{ColumnRef, ExprNode};
use super::super::extension::ExprRenderCtx;
use super::super::handles::{ColumnOwner, Table};
use super::super::source::Source;
use super::super::validate::{
    is_model_root, source_alias, validate_identifier, validate_reference, validate_source_identity,
    validate_table_source,
};
use super::{RenderMode, Renderer, SelectModel};
use crate::QueryError;

struct ManyToManyExists<'a> {
    from_through: &'a crate::ReferenceMeta,
    through_to: &'a crate::ReferenceMeta,
    through: &'a Table,
    target: &'a Table,
    predicate: Option<&'a ExprNode>,
    negated: bool,
}

impl Renderer {
    pub(in crate::queries) fn render_expr(
        &mut self,
        node: &ExprNode,
        mode: RenderMode<'_>,
    ) -> Result<String, QueryError> {
        match node {
            ExprNode::Column(column) => self.render_column(column, mode),
            ExprNode::Value(value) => self.render_value(value),
            ExprNode::Binary { left, op, right } => self.render_binary(left, op, right, mode),
            ExprNode::Unary { op, expr } => Ok(format!("{op} ({})", self.render_expr(expr, mode)?)),
            ExprNode::Bool { left, op, right } => Ok(format!(
                "({} {} {})",
                self.render_expr(left, mode)?,
                op,
                self.render_expr(right, mode)?
            )),
            ExprNode::Function { function, args } => {
                function.validate(self.dialect, args.len())?;
                let name = function.name(self.dialect)?;
                let rendered = args
                    .iter()
                    .map(|arg| self.render_expr(arg, mode))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(format!("{name}({})", rendered.join(", ")))
            }
            ExprNode::Custom { expression, args } => {
                expression.validate(self.dialect)?;
                let rendered = args
                    .iter()
                    .map(|arg| self.render_expr(arg, mode))
                    .collect::<Result<Vec<_>, _>>()?;
                let mut ctx = ExprRenderCtx::new(self.dialect, &rendered);
                expression.render(&mut ctx)
            }
            ExprNode::Over { expr, window } => {
                super::super::render_window::render_over(self, expr, window, mode)
            }
            ExprNode::InSource { left, source } => Ok(format!(
                "{} IN ({})",
                self.render_expr(left, mode)?,
                self.render_source_column_query(source)?
            )),
            ExprNode::InList { left, values } => {
                let rendered = values
                    .iter()
                    .map(|value| self.render_expr(value, mode))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(format!(
                    "{} IN ({})",
                    self.render_expr(left, mode)?,
                    rendered.join(", ")
                ))
            }
            ExprNode::RelationExists {
                reference,
                target,
                predicate,
                negated,
            } => {
                self.render_relation_exists(reference, target, predicate.as_deref(), *negated, mode)
            }
            ExprNode::RelationAggregate {
                function,
                reference,
                target,
                expr,
            } => self.render_relation_aggregate(function, reference, target, expr.as_deref(), mode),
            ExprNode::ManyToManyExists {
                from_through,
                through_to,
                through,
                target,
                predicate,
                negated,
            } => self.render_many_to_many_exists(
                ManyToManyExists {
                    from_through,
                    through_to,
                    through,
                    target,
                    predicate: predicate.as_deref(),
                    negated: *negated,
                },
                mode,
            ),
        }
    }

    fn render_relation_exists(
        &mut self,
        reference: &crate::ReferenceMeta,
        target: &Table,
        predicate: Option<&ExprNode>,
        negated: bool,
        mode: RenderMode<'_>,
    ) -> Result<String, QueryError> {
        let RenderMode::Select(parent) = mode else {
            return Err(QueryError::BindError(
                "relation predicates are only supported in read queries".to_string(),
            ));
        };
        validate_reference(reference)?;
        let alias = reference.logical_name;
        validate_identifier(alias)?;
        let mut sql = String::new();
        if negated {
            sql.push_str("NOT ");
        }
        sql.push_str("EXISTS (SELECT 1 FROM ");
        sql.push_str(&self.render_table_name(target)?);
        sql.push(' ');
        sql.push_str(alias);
        sql.push_str(" WHERE ");
        self.render_relation_on(reference, parent, alias, &mut sql)?;
        if let Some(predicate) = predicate {
            let child = relation_child_model(target, alias)?;
            sql.push_str(" AND ");
            sql.push_str(&self.render_expr(predicate, RenderMode::Select(&child))?);
        }
        sql.push(')');
        Ok(sql)
    }

    fn render_many_to_many_exists(
        &mut self,
        exists: ManyToManyExists<'_>,
        mode: RenderMode<'_>,
    ) -> Result<String, QueryError> {
        let RenderMode::Select(parent) = mode else {
            return Err(QueryError::BindError(
                "many-to-many predicates are only supported in read queries".to_string(),
            ));
        };
        let ManyToManyExists {
            from_through,
            through_to,
            through,
            target,
            predicate,
            negated,
        } = exists;
        validate_reference(from_through)?;
        validate_reference(through_to)?;
        let through_alias = from_through.logical_name;
        let target_alias = through_to.logical_name;
        let mut sql =
            many_to_many_head(self, through, target, through_alias, target_alias, negated)?;
        self.render_through_join(through_to, through_alias, target_alias, &mut sql);
        sql.push_str(" WHERE ");
        self.render_relation_on(from_through, parent, through_alias, &mut sql)?;
        if let Some(predicate) = predicate {
            let child = relation_child_model(target, target_alias)?;
            sql.push_str(" AND ");
            sql.push_str(&self.render_expr(predicate, RenderMode::Select(&child))?);
        }
        sql.push(')');
        Ok(sql)
    }

    fn render_through_join(
        &self,
        reference: &crate::ReferenceMeta,
        through_alias: &str,
        target_alias: &str,
        sql: &mut String,
    ) {
        for (idx, column) in reference.columns.iter().enumerate() {
            if idx > 0 {
                sql.push_str(" AND ");
            }
            sql.push_str(target_alias);
            sql.push('.');
            sql.push_str(column.to);
            sql.push_str(" = ");
            sql.push_str(through_alias);
            sql.push('.');
            sql.push_str(column.from);
        }
    }

    fn render_relation_aggregate(
        &mut self,
        function: &str,
        reference: &crate::ReferenceMeta,
        target: &Table,
        expr: Option<&ExprNode>,
        mode: RenderMode<'_>,
    ) -> Result<String, QueryError> {
        let RenderMode::Select(parent) = mode else {
            return Err(QueryError::BindError(
                "relation aggregates are only supported in read queries".to_string(),
            ));
        };
        validate_reference(reference)?;
        let alias = reference.logical_name;
        validate_identifier(alias)?;
        let child = relation_child_model(target, alias)?;
        let rendered = match expr {
            Some(expr) => self.render_expr(expr, RenderMode::Select(&child))?,
            None => "*".to_string(),
        };
        let mut sql = format!(
            "(SELECT {function}({rendered}) FROM {} {} WHERE ",
            self.render_table_name(target)?,
            alias
        );
        self.render_relation_on(reference, parent, alias, &mut sql)?;
        sql.push(')');
        Ok(sql)
    }

    fn render_relation_on(
        &self,
        reference: &crate::ReferenceMeta,
        parent: &SelectModel,
        alias: &str,
        sql: &mut String,
    ) -> Result<(), QueryError> {
        for (idx, column) in reference.columns.iter().enumerate() {
            if idx > 0 {
                sql.push_str(" AND ");
            }
            sql.push_str(alias);
            sql.push('.');
            sql.push_str(column.to);
            sql.push_str(" = ");
            sql.push_str(&self.resolve_model_column(column.from, parent)?);
        }
        Ok(())
    }

    fn render_binary(
        &mut self,
        left: &ExprNode,
        op: &str,
        right: &ExprNode,
        mode: RenderMode<'_>,
    ) -> Result<String, QueryError> {
        self.validate_operator(op)?;
        Ok(format!(
            "({} {} {})",
            self.render_expr(left, mode)?,
            op,
            self.render_expr(right, mode)?
        ))
    }

    fn render_column(
        &self,
        column: &ColumnRef,
        mode: RenderMode<'_>,
    ) -> Result<String, QueryError> {
        validate_identifier(&column.name)?;
        match (mode, &column.owner) {
            (RenderMode::Select(model), ColumnOwner::Root(table)) => {
                validate_table_source(table, &model.source)?;
                Ok(format!("{}.{}", model.root_alias, column.name))
            }
            (RenderMode::Select(model), ColumnOwner::Source(source)) => {
                validate_source_identity(source, &model.source)?;
                Ok(format!("{}.{}", model.root_alias, column.name))
            }
            (RenderMode::Select(model), ColumnOwner::Reference(reference)) => {
                if is_model_root(reference, model) {
                    return Ok(format!("{}.{}", model.root_alias, column.name));
                }
                if !model.references.contains_key(reference.as_ref()) {
                    return Err(QueryError::UnknownAlias(reference.to_string()));
                }
                Ok(format!("{reference}.{}", column.name))
            }
            (RenderMode::MutationRoot { source }, ColumnOwner::Root(table)) => {
                validate_table_source(table, source)?;
                Ok(column.name.to_string())
            }
            (RenderMode::MutationRoot { .. }, ColumnOwner::Source(source)) => {
                Err(QueryError::BindError(format!(
                    "mutation filters do not support source column '{}'",
                    source
                )))
            }
            (RenderMode::MutationRoot { .. }, ColumnOwner::Reference(reference)) => {
                Err(QueryError::BindError(format!(
                    "mutation filters do not support reference column '{}'",
                    reference
                )))
            }
        }
    }

    fn validate_operator(&self, op: &str) -> Result<(), QueryError> {
        if op == "ILIKE" {
            self.dialect_renderer
                .validate_feature(DialectFeature::Ilike)?;
        }
        Ok(())
    }
}

fn relation_child_model(target: &Table, alias: &str) -> Result<SelectModel, QueryError> {
    validate_identifier(alias)?;
    Ok(SelectModel {
        source: Source::Table(target.clone()),
        root_alias: alias.to_string(),
        scan_root_alias: source_alias(&Source::Table(target.clone()), alias),
        references: indexmap::IndexMap::new(),
        columns: Vec::new(),
        result_type: "",
    })
}

fn many_to_many_head(
    renderer: &Renderer,
    through: &Table,
    target: &Table,
    through_alias: &str,
    target_alias: &str,
    negated: bool,
) -> Result<String, QueryError> {
    validate_identifier(through_alias)?;
    validate_identifier(target_alias)?;
    let mut sql = String::new();
    if negated {
        sql.push_str("NOT ");
    }
    sql.push_str("EXISTS (SELECT 1 FROM ");
    sql.push_str(&renderer.render_table_name(through)?);
    sql.push(' ');
    sql.push_str(through_alias);
    sql.push_str(" JOIN ");
    sql.push_str(&renderer.render_table_name(target)?);
    sql.push(' ');
    sql.push_str(target_alias);
    sql.push_str(" ON ");
    Ok(sql)
}
