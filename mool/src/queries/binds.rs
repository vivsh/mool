//! Parameter planning and SQLx statement binding for typed queries.

use std::any::TypeId;
use std::collections::{HashMap, HashSet};

use crate::argvalue::ArgValue;
use crate::commons::Arguments;
use crate::interfaces::Record;

use super::super::{QueryError, Statement};
use super::GENERATED_PREFIX;
use super::expr::{ColumnRef, ExprNode};
use super::handles::{Table, VarId};
use super::output::SelectAssignment;
use super::plan::{ParamSource, QueryPlan};
use super::source::{CteSource, Source};
use super::validate::{output_column, table_name, validate_identifier};

pub(super) fn statement_from_plan(
    plan: QueryPlan,
    mut args: Arguments<'static>,
) -> Result<Statement, QueryError> {
    validate_unused_binds(&plan)?;
    for name in &plan.bind_order {
        let value = plan
            .values
            .get(name)
            .ok_or_else(|| QueryError::MissingBinding(name.clone()))?;
        value
            .bind_value(&mut args)
            .map_err(|err| QueryError::BindError(err.to_string()))?;
    }
    Ok(Statement::new(&plan.sql, args))
}

pub(super) fn finish_plan(plan: QueryPlan) -> Result<QueryPlan, QueryError> {
    validate_plan_invariants(&plan)?;
    validate_missing_binds(&plan)?;
    validate_unused_binds(&plan)?;
    Ok(plan)
}

/// Validates bind metadata shared by planning, execution, and public inspection.
fn validate_plan_invariants(plan: &QueryPlan) -> Result<(), QueryError> {
    let dynamic = plan.bind_order.len();
    if plan.dynamic_bind_count != dynamic {
        return Err(QueryError::BindCountMismatch {
            expected: dynamic,
            got: plan.dynamic_bind_count,
        });
    }
    let total = plan
        .prebound_count
        .checked_add(dynamic)
        .ok_or_else(|| QueryError::BindError("planned bind count overflow".to_string()))?;
    if plan.total_bind_count != total {
        return Err(QueryError::BindCountMismatch {
            expected: total,
            got: plan.total_bind_count,
        });
    }
    validate_bind_positions(plan)
}

/// Ensures every generated bind slot is represented exactly in parameter metadata.
fn validate_bind_positions(plan: &QueryPlan) -> Result<(), QueryError> {
    let mut positions = HashSet::with_capacity(plan.dynamic_bind_count);
    for (offset, name) in plan.bind_order.iter().enumerate() {
        let expected = plan.prebound_count + offset + 1;
        let spec = plan.params.get(name).ok_or_else(|| {
            QueryError::BindError(format!("missing parameter metadata for '{name}'"))
        })?;
        if !spec.occurrences.contains(&expected) {
            return Err(QueryError::BindError(format!(
                "parameter '{name}' is missing planned position {expected}"
            )));
        }
        positions.insert(expected);
    }
    validate_param_occurrences(plan, &positions)
}

/// Rejects invalid first positions and occurrence references outside planned slots.
fn validate_param_occurrences(
    plan: &QueryPlan,
    positions: &HashSet<usize>,
) -> Result<(), QueryError> {
    for spec in plan.params.values() {
        if spec.occurrences.first() != Some(&spec.position) {
            return Err(QueryError::BindError(format!(
                "parameter '{}' has inconsistent first position",
                spec.name
            )));
        }
        if spec
            .occurrences
            .iter()
            .any(|position| !positions.contains(position))
        {
            return Err(QueryError::BindError(format!(
                "parameter '{}' references an unplanned bind position",
                spec.name
            )));
        }
    }
    Ok(())
}

fn validate_missing_binds(plan: &QueryPlan) -> Result<(), QueryError> {
    for (name, spec) in &plan.params {
        if spec.source == ParamSource::Var && !plan.values.contains_key(name) {
            return Err(QueryError::MissingBinding(
                spec.display_name
                    .clone()
                    .unwrap_or_else(|| spec.name.clone()),
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_unused_binds(plan: &QueryPlan) -> Result<(), QueryError> {
    for name in plan.values.keys() {
        if name.starts_with(GENERATED_PREFIX) {
            continue;
        }
        match plan.params.get(name) {
            Some(spec) if spec.source == ParamSource::Var => {}
            Some(_) => {}
            None => return Err(QueryError::UnusedBinding(name.clone())),
        }
    }
    Ok(())
}

pub(super) fn collect_expr_binds(
    node: &ExprNode,
    values: &mut HashMap<VarId, ArgValue>,
) -> Result<(), QueryError> {
    match node {
        ExprNode::Column(_) | ExprNode::Value(_) => Ok(()),
        ExprNode::Binary { left, right, .. } | ExprNode::Bool { left, right, .. } => {
            collect_expr_binds(left, values)?;
            collect_expr_binds(right, values)
        }
        ExprNode::Unary { expr, .. } | ExprNode::NullCheck { expr, .. } => {
            collect_expr_binds(expr, values)
        }
        ExprNode::Between {
            expr, lower, upper, ..
        } => {
            collect_expr_binds(expr, values)?;
            collect_expr_binds(lower, values)?;
            collect_expr_binds(upper, values)
        }
        ExprNode::Function { args, .. } | ExprNode::Custom { args, .. } => {
            for arg in args {
                collect_expr_binds(arg, values)?;
            }
            Ok(())
        }
        ExprNode::Over { expr, window } => {
            collect_expr_binds(expr, values)?;
            collect_window_binds(window, values)
        }
        ExprNode::InSource { left, source } => {
            collect_expr_binds(left, values)?;
            collect_source_binds(&source.source, values)
        }
        ExprNode::InList {
            left, values: list, ..
        } => {
            collect_expr_binds(left, values)?;
            for value in list {
                collect_expr_binds(value, values)?;
            }
            Ok(())
        }
        ExprNode::RelationExists { predicate, .. } => {
            if let Some(predicate) = predicate {
                collect_expr_binds(predicate, values)?;
            }
            Ok(())
        }
        ExprNode::RelationAggregate { expr, .. } => {
            if let Some(expr) = expr {
                collect_expr_binds(expr, values)?;
            }
            Ok(())
        }
        ExprNode::ManyToManyExists { predicate, .. } => {
            if let Some(predicate) = predicate {
                collect_expr_binds(predicate, values)?;
            }
            Ok(())
        }
    }
}

pub(super) fn collect_source_binds(
    source: &Source,
    values: &mut HashMap<VarId, ArgValue>,
) -> Result<(), QueryError> {
    match source {
        Source::Subquery(subquery) => subquery.data.scope.collect_binds_into(values),
        Source::Cte(_) | Source::Table(_) => Ok(()),
    }
}

pub(super) fn collect_source_ctes(source: &Source, used: &mut HashSet<CteSource>) {
    if let Source::Cte(cte) = source {
        used.insert(cte.clone());
    }
}

pub(super) fn collect_expr_ctes(node: &ExprNode, used: &mut HashSet<CteSource>) {
    match node {
        ExprNode::Column(_) | ExprNode::Value(_) => {}
        ExprNode::Binary { left, right, .. } | ExprNode::Bool { left, right, .. } => {
            collect_expr_ctes(left, used);
            collect_expr_ctes(right, used);
        }
        ExprNode::Unary { expr, .. } | ExprNode::NullCheck { expr, .. } => {
            collect_expr_ctes(expr, used)
        }
        ExprNode::Between {
            expr, lower, upper, ..
        } => {
            collect_expr_ctes(expr, used);
            collect_expr_ctes(lower, used);
            collect_expr_ctes(upper, used);
        }
        ExprNode::Function { args, .. } | ExprNode::Custom { args, .. } => {
            for arg in args {
                collect_expr_ctes(arg, used);
            }
        }
        ExprNode::Over { expr, window } => {
            collect_expr_ctes(expr, used);
            collect_window_ctes(window, used);
        }
        ExprNode::InSource { left, source } => {
            collect_expr_ctes(left, used);
            collect_source_ctes(&source.source, used);
        }
        ExprNode::InList { left, values, .. } => {
            collect_expr_ctes(left, used);
            for value in values {
                collect_expr_ctes(value, used);
            }
        }
        ExprNode::RelationExists { predicate, .. } => {
            if let Some(predicate) = predicate {
                collect_expr_ctes(predicate, used);
            }
        }
        ExprNode::RelationAggregate { expr, .. } => {
            if let Some(expr) = expr {
                collect_expr_ctes(expr, used);
            }
        }
        ExprNode::ManyToManyExists { predicate, .. } => {
            if let Some(predicate) = predicate {
                collect_expr_ctes(predicate, used);
            }
        }
    }
}

fn collect_window_binds(
    window: &super::window::WindowSpec,
    values: &mut HashMap<VarId, ArgValue>,
) -> Result<(), QueryError> {
    for expr in &window.partitions {
        collect_expr_binds(expr, values)?;
    }
    for order in &window.orders {
        collect_expr_binds(&order.expr, values)?;
    }
    if let Some(frame) = &window.frame {
        collect_bound_binds(&frame.start, values)?;
        collect_bound_binds(&frame.end, values)?;
    }
    Ok(())
}

fn collect_bound_binds(
    bound: &super::window::FrameBound,
    values: &mut HashMap<VarId, ArgValue>,
) -> Result<(), QueryError> {
    if let Some(expr) = &bound.expr {
        collect_expr_binds(expr, values)?;
    }
    Ok(())
}

fn collect_window_ctes(window: &super::window::WindowSpec, used: &mut HashSet<CteSource>) {
    for expr in &window.partitions {
        collect_expr_ctes(expr, used);
    }
    for order in &window.orders {
        collect_expr_ctes(&order.expr, used);
    }
    if let Some(frame) = &window.frame {
        collect_bound_ctes(&frame.start, used);
        collect_bound_ctes(&frame.end, used);
    }
}

fn collect_bound_ctes(bound: &super::window::FrameBound, used: &mut HashSet<CteSource>) {
    if let Some(expr) = &bound.expr {
        collect_expr_ctes(expr, used);
    }
}

pub(super) fn validate_cte_usage(
    defined: &indexmap::IndexMap<String, CteSource>,
    used: &HashSet<CteSource>,
    named_used: &HashSet<String>,
) -> Result<(), QueryError> {
    for (name, source) in defined {
        if !used.contains(source) && !named_used.contains(name) {
            return Err(QueryError::BindError(format!("unused CTE '{}'", name)));
        }
    }
    for source in used {
        if !defined.values().any(|defined| defined == source) {
            return Err(QueryError::BindError(format!(
                "CTE '{}' is not registered",
                source.data.name
            )));
        }
    }
    for name in named_used {
        if !defined.contains_key(name) {
            return Err(QueryError::BindError(format!(
                "CTE '{}' is not registered",
                name
            )));
        }
    }
    Ok(())
}

pub(super) fn insert_bind(
    values: &mut HashMap<VarId, ArgValue>,
    id: VarId,
    value: ArgValue,
) -> Result<(), QueryError> {
    if values.contains_key(&id) {
        return Err(QueryError::BindError(format!(
            "duplicate binding for var {}",
            id.value()
        )));
    }
    values.insert(id, value);
    Ok(())
}

pub(super) fn validate_output_assignments(
    exprs: &[SelectAssignment],
    columns: &[String],
    record: TypeId,
    record_name: &'static str,
) -> Result<(), QueryError> {
    let mut seen = HashSet::new();
    for assignment in exprs {
        let name = assignment.target.name.as_ref();
        validate_identifier(name).map_err(|_| QueryError::InvalidProjection(name.to_string()))?;
        validate_output_record(assignment, record, record_name)?;
        let target_exists = columns.iter().any(|column| {
            column == name || matches!(output_column(column), Ok(output) if output == name)
        });
        if !target_exists || !seen.insert(name.to_string()) {
            return Err(QueryError::InvalidProjection(name.to_string()));
        }
    }
    Ok(())
}

fn validate_output_record(
    assignment: &SelectAssignment,
    record: TypeId,
    record_name: &'static str,
) -> Result<(), QueryError> {
    if assignment.target.record == record {
        return Ok(());
    }
    Err(QueryError::InvalidProjection(format!(
        "{} belongs to {}, expected {}",
        assignment.target.name, assignment.target.record_name, record_name
    )))
}

pub(super) fn insert_columns<T>() -> Result<Vec<String>, QueryError>
where
    T: Record,
{
    let columns = T::record_insert_column_names();
    if columns.is_empty() {
        return Err(QueryError::BindError("no bindable columns".to_string()));
    }
    for column in &columns {
        validate_identifier(column)?;
    }
    Ok(columns)
}

pub(super) fn validate_bind_columns(table: &Table, columns: &[String]) -> Result<(), QueryError> {
    let Some(known) = table.data.columns.as_ref() else {
        return Ok(());
    };
    for column in columns {
        if !known.iter().any(|known| known == column) {
            return Err(QueryError::BindError(format!(
                "column '{}' is not writable for {}",
                column,
                table_name(table.data.schema.as_deref(), table.data.name.as_ref())
            )));
        }
    }
    Ok(())
}

pub(super) fn upsert_update_columns<'a>(
    columns: &'a [String],
    conflict: &[ColumnRef],
) -> Result<Vec<&'a str>, QueryError> {
    let mut update_columns = Vec::with_capacity(columns.len());
    for column in columns {
        validate_identifier(column)?;
        if !conflict
            .iter()
            .any(|conflict| conflict.name.as_ref() == column)
        {
            update_columns.push(column.as_str());
        }
    }
    Ok(update_columns)
}

pub(super) fn bind_insert_rows<T>(
    rows: &[T],
    col_count: usize,
) -> Result<Arguments<'static>, QueryError>
where
    T: Record,
{
    if rows.is_empty() {
        return Err(QueryError::BindError(
            "cannot insert empty list".to_string(),
        ));
    }
    let mut args = Arguments::default();
    for row in rows {
        bind_insert_row(row, col_count, &mut args)?;
    }
    Ok(args)
}

pub(super) fn bind_update_rows<T>(
    rows: &[T],
    columns: &[&str],
) -> Result<Arguments<'static>, QueryError>
where
    T: Record,
{
    use sqlx::Arguments as _;

    let mut args = Arguments::default();
    for row in rows {
        let before = args.len();
        row.record_bind_update_selected(columns, &mut args)
            .map_err(|error| QueryError::BindError(error.to_string()))?;
        let added = args.len().saturating_sub(before);
        if added != columns.len() {
            return Err(QueryError::BindCountMismatch {
                expected: columns.len(),
                got: added,
            });
        }
    }
    Ok(args)
}

pub(super) fn bind_insert_row<T>(
    row: &T,
    col_count: usize,
    args: &mut Arguments<'static>,
) -> Result<(), QueryError>
where
    T: Record,
{
    use sqlx::Arguments as _;

    let before = args.len();
    row.record_bind_insert_values(args)
        .map_err(|err| QueryError::BindError(err.to_string()))?;
    let bound = args.len().saturating_sub(before);
    if bound != col_count {
        return Err(QueryError::BindCountMismatch {
            expected: col_count,
            got: bound,
        });
    }
    Ok(())
}
