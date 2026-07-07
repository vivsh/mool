//! Query planning and validation for typed query scopes.

use indexmap::IndexMap;
use std::any::{TypeId, type_name};
use std::collections::{HashMap, HashSet};

use crate::argvalue::ArgValue;
use crate::commons::Arguments;
use crate::interfaces::Record;
use crate::placeholders::Dialect;
use crate::relations::ReferenceMeta;

use super::binds::{
    bind_columns, bind_rows, collect_expr_binds, collect_expr_ctes, collect_source_ctes,
    finish_plan, insert_bind, validate_bind_columns, validate_cte_usage,
    validate_output_assignments,
};
use super::expr::ExprNode;
use super::handles::VarId;
use super::plan::QueryPlan;
use super::render::{Renderer, SelectModel};
use super::scope::QueryScope;
use super::set::SetOp;
use super::source::SelectSource;
use super::traits::IntoColumnRef;
use super::validate::{
    output_columns, reject_window, source_table, validate_conflict_columns, validate_expr_owners,
    validate_identifier,
};
use super::values::WriteInput;
use crate::QueryError;

impl QueryScope {
    pub(super) fn plan_all<T>(&self, dialect: Dialect) -> Result<QueryPlan, QueryError>
    where
        T: Record + 'static,
    {
        self.plan_select::<T>(None, dialect)
    }

    pub(super) fn plan_first<T>(&self, dialect: Dialect) -> Result<QueryPlan, QueryError>
    where
        T: Record + 'static,
    {
        self.plan_select::<T>(Some((0, 1)), dialect)
    }

    pub(super) fn plan_one<T>(&self, dialect: Dialect) -> Result<QueryPlan, QueryError>
    where
        T: Record + 'static,
    {
        self.plan_select::<T>(Some((0, 2)), dialect)
    }

    pub(super) fn plan_slice<T>(
        &self,
        offset: usize,
        count: usize,
        dialect: Dialect,
    ) -> Result<QueryPlan, QueryError>
    where
        T: Record + 'static,
    {
        self.plan_select::<T>(Some((offset, count)), dialect)
    }

    pub(super) fn plan_insert<W>(&self, row: &W, dialect: Dialect) -> Result<QueryPlan, QueryError>
    where
        W: WriteInput,
    {
        self.plan_insert_with_args(row, dialect)
            .map(|(plan, _)| plan)
    }

    pub(super) fn plan_update<W>(&self, row: &W, dialect: Dialect) -> Result<QueryPlan, QueryError>
    where
        W: WriteInput,
    {
        self.plan_update_with_args(row, dialect)
            .map(|(plan, _)| plan)
    }

    pub(super) fn plan_delete(&self, dialect: Dialect) -> Result<QueryPlan, QueryError> {
        self.validate_scope_errors()?;
        self.validate_mutation_filters()?;
        let mut renderer = Renderer::new(dialect);
        let sql = renderer.render_delete(self, None)?;
        finish_plan(renderer.plan(sql, None, self.collect_binds()?))
    }

    pub(super) fn plan_count(&self, dialect: Dialect) -> Result<QueryPlan, QueryError> {
        self.validate_scope_errors()?;
        let model = SelectModel::source_only(&self.source)?;
        self.validate_aggregate(&model, None)?;
        let mut renderer = Renderer::new(dialect);
        let sql = renderer.render_count(self, &model)?;
        finish_plan(renderer.plan(sql, None, self.collect_binds()?))
    }

    pub(super) fn plan_exists(&self, dialect: Dialect) -> Result<QueryPlan, QueryError> {
        self.validate_scope_errors()?;
        let model = SelectModel::source_only(&self.source)?;
        self.validate_aggregate(&model, None)?;
        let mut renderer = Renderer::new(dialect);
        let sql = renderer.render_exists(self, &model)?;
        finish_plan(renderer.plan(sql, None, self.collect_binds()?))
    }

    pub(super) fn plan_scalar(
        &self,
        expr: ExprNode,
        dialect: Dialect,
    ) -> Result<QueryPlan, QueryError> {
        self.validate_scope_errors()?;
        let model = SelectModel::source_only(&self.source)?;
        self.validate_aggregate(&model, Some(&expr))?;
        let mut renderer = Renderer::new(dialect);
        let sql = renderer.render_scalar(self, &model, &expr)?;
        finish_plan(renderer.plan(sql, None, self.collect_binds()?))
    }

    pub(super) fn plan_batch_insert<T>(
        &self,
        rows: &[T],
        dialect: Dialect,
    ) -> Result<QueryPlan, QueryError>
    where
        T: Record + 'static,
    {
        self.plan_batch_insert_with_args(rows, dialect)
            .map(|(plan, _)| plan)
    }

    pub(super) fn plan_batch_upsert<T, I, C>(
        &self,
        rows: &[T],
        conflict: I,
        dialect: Dialect,
    ) -> Result<QueryPlan, QueryError>
    where
        T: Record + 'static,
        I: IntoIterator<Item = C>,
        C: IntoColumnRef,
    {
        self.plan_batch_upsert_with_args(rows, conflict, dialect)
            .map(|(plan, _)| plan)
    }

    fn plan_select<T>(
        &self,
        slice: Option<(usize, usize)>,
        dialect: Dialect,
    ) -> Result<QueryPlan, QueryError>
    where
        T: Record + 'static,
    {
        self.validate_scope_errors()?;
        let model = SelectModel::new::<T>(&self.source)?;
        self.validate_columns_for_select::<T>(&model)?;
        let mut renderer = Renderer::new(dialect);
        let sql = renderer.render_select(self, &model, slice)?;
        finish_plan(renderer.plan(sql, Some(model.result_type), self.collect_binds()?))
    }

    pub(super) fn plan_set_all<T>(
        &self,
        rhs: &Self,
        op: SetOp,
        extra_binds: &HashMap<VarId, ArgValue>,
        extra_errors: &[QueryError],
        dialect: Dialect,
    ) -> Result<QueryPlan, QueryError>
    where
        T: Record + 'static,
    {
        self.validate_set_operand()?;
        rhs.validate_set_operand()?;
        if let Some(error) = extra_errors.first() {
            return Err(error.clone());
        }

        let left_model = SelectModel::new::<T>(&self.source)?;
        self.validate_columns_for_select::<T>(&left_model)?;
        let right_model = SelectModel::new::<T>(&rhs.source)?;
        rhs.validate_columns_for_select::<T>(&right_model)?;

        let mut renderer = Renderer::new(dialect);
        let left_sql = renderer.render_select(self, &left_model, None)?;
        let right_sql = renderer.render_select(rhs, &right_model, None)?;
        let sql = format!("{left_sql} {} {right_sql}", op.sql());
        let mut binds = self.collect_binds()?;
        rhs.collect_binds_into(&mut binds)?;
        for (id, value) in extra_binds {
            insert_bind(&mut binds, *id, value.clone())?;
        }
        finish_plan(renderer.plan(sql, Some(left_model.result_type), binds))
    }

    pub(super) fn select_source<T>(
        &self,
        name: &str,
        slice: Option<(usize, usize)>,
    ) -> Result<SelectSource, QueryError>
    where
        T: Record + 'static,
    {
        validate_identifier(name)?;
        self.validate_scope_errors()?;
        let model = SelectModel::new::<T>(&self.source)?;
        self.validate_columns_for_select::<T>(&model)?;
        Ok(SelectSource {
            columns: output_columns(&model.columns)?,
            model,
            slice,
        })
    }

    pub(super) fn plan_insert_with_args<W>(
        &self,
        row: &W,
        dialect: Dialect,
    ) -> Result<(QueryPlan, Arguments<'static>), QueryError>
    where
        W: WriteInput,
    {
        self.validate_scope_errors()?;
        self.validate_insert_scope()?;
        let parts = row.write_parts(source_table(&self.source)?)?;
        let mut renderer = Renderer::with_prebound(dialect, parts.prebound_count);
        let sql = renderer.render_insert(self, &parts, false, &[], None)?;
        Ok((
            finish_plan(renderer.plan(sql, None, self.collect_binds()?))?,
            parts.args,
        ))
    }

    pub(super) fn plan_update_with_args<W>(
        &self,
        row: &W,
        dialect: Dialect,
    ) -> Result<(QueryPlan, Arguments<'static>), QueryError>
    where
        W: WriteInput,
    {
        self.validate_scope_errors()?;
        self.validate_update_scope()?;
        let parts = row.write_parts(source_table(&self.source)?)?;
        let mut renderer = Renderer::with_prebound(dialect, parts.prebound_count);
        let sql = renderer.render_update(self, &parts, None)?;
        Ok((
            finish_plan(renderer.plan(sql, None, self.collect_binds()?))?,
            parts.args,
        ))
    }

    pub(super) fn plan_batch_insert_with_args<T>(
        &self,
        rows: &[T],
        dialect: Dialect,
    ) -> Result<(QueryPlan, Arguments<'static>), QueryError>
    where
        T: Record + 'static,
    {
        self.validate_scope_errors()?;
        self.validate_insert_scope()?;
        let columns = bind_columns::<T>()?;
        validate_bind_columns(source_table(&self.source)?, &columns)?;
        let args = bind_rows(rows, columns.len())?;
        let mut renderer = Renderer::with_prebound(dialect, rows.len() * columns.len());
        let sql = renderer.render_batch_insert(self, &columns, rows.len(), false, &[], None)?;
        Ok((
            finish_plan(renderer.plan(sql, None, self.collect_binds()?))?,
            args,
        ))
    }

    pub(super) fn plan_batch_upsert_with_args<T, I, C>(
        &self,
        rows: &[T],
        conflict: I,
        dialect: Dialect,
    ) -> Result<(QueryPlan, Arguments<'static>), QueryError>
    where
        T: Record + 'static,
        I: IntoIterator<Item = C>,
        C: IntoColumnRef,
    {
        self.validate_scope_errors()?;
        self.validate_insert_scope()?;
        let columns = bind_columns::<T>()?;
        validate_bind_columns(source_table(&self.source)?, &columns)?;
        let conflict = conflict
            .into_iter()
            .map(IntoColumnRef::into_column_ref)
            .collect::<Vec<_>>();
        if conflict.is_empty() {
            return Err(QueryError::BindError(
                "batch_upsert requires conflict columns".to_string(),
            ));
        }
        validate_conflict_columns(&conflict, source_table(&self.source)?)?;
        let args = bind_rows(rows, columns.len())?;
        let mut renderer = Renderer::with_prebound(dialect, rows.len() * columns.len());
        let sql =
            renderer.render_batch_insert(self, &columns, rows.len(), true, &conflict, None)?;
        Ok((
            finish_plan(renderer.plan(sql, None, self.collect_binds()?))?,
            args,
        ))
    }

    pub(super) fn plan_insert_returning<W>(
        &self,
        row: &W,
        dialect: Dialect,
        returning: &SelectModel,
    ) -> Result<(QueryPlan, Arguments<'static>), QueryError>
    where
        W: WriteInput,
    {
        self.validate_scope_errors()?;
        self.validate_insert_returning_scope()?;
        let parts = row.write_parts(source_table(&self.source)?)?;
        let mut renderer = Renderer::with_prebound(dialect, parts.prebound_count);
        let sql = renderer.render_insert(self, &parts, false, &[], Some(returning))?;
        Ok((self.finish_returning(renderer, sql, returning)?, parts.args))
    }

    pub(super) fn plan_update_returning<W>(
        &self,
        row: &W,
        dialect: Dialect,
        returning: &SelectModel,
    ) -> Result<(QueryPlan, Arguments<'static>), QueryError>
    where
        W: WriteInput,
    {
        self.validate_scope_errors()?;
        self.validate_update_returning_scope()?;
        let parts = row.write_parts(source_table(&self.source)?)?;
        let mut renderer = Renderer::with_prebound(dialect, parts.prebound_count);
        let sql = renderer.render_update(self, &parts, Some(returning))?;
        Ok((self.finish_returning(renderer, sql, returning)?, parts.args))
    }

    pub(super) fn plan_delete_returning(
        &self,
        dialect: Dialect,
        returning: &SelectModel,
    ) -> Result<QueryPlan, QueryError> {
        self.validate_scope_errors()?;
        self.validate_mutation_filters()?;
        let mut renderer = Renderer::new(dialect);
        let sql = renderer.render_delete(self, Some(returning))?;
        self.finish_returning(renderer, sql, returning)
    }

    pub(super) fn plan_batch_insert_returning<T>(
        &self,
        rows: &[T],
        dialect: Dialect,
        returning: &SelectModel,
    ) -> Result<(QueryPlan, Arguments<'static>), QueryError>
    where
        T: Record + 'static,
    {
        self.validate_scope_errors()?;
        self.validate_insert_scope()?;
        let columns = bind_columns::<T>()?;
        validate_bind_columns(source_table(&self.source)?, &columns)?;
        let args = bind_rows(rows, columns.len())?;
        let mut renderer = Renderer::with_prebound(dialect, rows.len() * columns.len());
        let sql = renderer.render_batch_insert(
            self,
            &columns,
            rows.len(),
            false,
            &[],
            Some(returning),
        )?;
        Ok((self.finish_returning(renderer, sql, returning)?, args))
    }

    pub(super) fn plan_batch_upsert_returning<T, I, C>(
        &self,
        rows: &[T],
        conflict: I,
        dialect: Dialect,
        returning: &SelectModel,
    ) -> Result<(QueryPlan, Arguments<'static>), QueryError>
    where
        T: Record + 'static,
        I: IntoIterator<Item = C>,
        C: IntoColumnRef,
    {
        self.validate_scope_errors()?;
        self.validate_insert_scope()?;
        let columns = bind_columns::<T>()?;
        validate_bind_columns(source_table(&self.source)?, &columns)?;
        let conflict = conflict
            .into_iter()
            .map(IntoColumnRef::into_column_ref)
            .collect::<Vec<_>>();
        if conflict.is_empty() {
            return Err(QueryError::BindError(
                "batch_upsert requires conflict columns".to_string(),
            ));
        }
        validate_conflict_columns(&conflict, source_table(&self.source)?)?;
        let args = bind_rows(rows, columns.len())?;
        let mut renderer = Renderer::with_prebound(dialect, rows.len() * columns.len());
        let sql = renderer.render_batch_insert(
            self,
            &columns,
            rows.len(),
            true,
            &conflict,
            Some(returning),
        )?;
        Ok((self.finish_returning(renderer, sql, returning)?, args))
    }

    fn finish_returning(
        &self,
        renderer: Renderer,
        sql: String,
        returning: &SelectModel,
    ) -> Result<QueryPlan, QueryError> {
        finish_plan(renderer.plan(sql, Some(returning.result_type), self.collect_binds()?))
    }

    fn validate_columns_for_select<T>(&self, model: &SelectModel) -> Result<(), QueryError>
    where
        T: Record + 'static,
    {
        validate_output_assignments(
            &self.output_assignments,
            &model.columns,
            TypeId::of::<T>(),
            type_name::<T>(),
        )?;
        self.validate_read_windows()?;
        self.validate_ctes(Some(&model.references))?;
        let mut refs = HashSet::new();
        for key in model.references.keys() {
            refs.insert(key.as_str());
        }
        for node in self.expression_nodes() {
            validate_expr_owners(
                node,
                &self.source,
                Some(&refs),
                Some(&model.scan_root_alias),
                true,
            )?;
        }
        Ok(())
    }

    fn validate_scope_errors(&self) -> Result<(), QueryError> {
        if let Some(error) = self.errors.first() {
            return Err(error.clone());
        }
        Ok(())
    }

    /// Validates aggregate terminals (`count`/`exists`/`scalar`) over a
    /// projection-free source model. Only root columns are addressable.
    fn validate_aggregate(
        &self,
        model: &SelectModel,
        extra: Option<&ExprNode>,
    ) -> Result<(), QueryError> {
        self.validate_ctes(Some(&model.references))?;
        self.validate_aggregate_windows(extra)?;
        let refs: HashSet<&str> = HashSet::new();
        for node in self.expression_nodes().chain(extra) {
            validate_expr_owners(
                node,
                &self.source,
                Some(&refs),
                Some(&model.scan_root_alias),
                true,
            )?;
        }
        Ok(())
    }

    fn validate_mutation_filters(&self) -> Result<(), QueryError> {
        if self.filters.is_empty() {
            return Err(QueryError::BindError(
                "update and delete require at least one filter".to_string(),
            ));
        }
        self.validate_ctes(None)?;
        for node in self.expression_nodes() {
            reject_window(node, "mutation statements")?;
            validate_expr_owners(node, &self.source, None, None, false)?;
        }
        Ok(())
    }

    fn validate_set_operand(&self) -> Result<(), QueryError> {
        self.validate_scope_errors()?;
        if !self.ctes.is_empty() {
            return Err(QueryError::BindError(
                "set operation operands do not support local CTEs".to_string(),
            ));
        }
        if !self.orders.is_empty() {
            return Err(QueryError::BindError(
                "set operation operands do not support order_by".to_string(),
            ));
        }
        Ok(())
    }

    fn validate_read_windows(&self) -> Result<(), QueryError> {
        for filter in &self.filters {
            reject_window(&filter.node, "WHERE")?;
        }
        for group in &self.groups {
            reject_window(group, "GROUP BY")?;
        }
        for predicate in &self.having {
            reject_window(&predicate.node, "HAVING")?;
        }
        Ok(())
    }

    fn validate_aggregate_windows(&self, extra: Option<&ExprNode>) -> Result<(), QueryError> {
        self.validate_read_windows()?;
        if let Some(expr) = extra {
            reject_window(expr, "scalar terminals")?;
        }
        Ok(())
    }

    fn validate_ctes(
        &self,
        references: Option<&IndexMap<String, ReferenceMeta>>,
    ) -> Result<(), QueryError> {
        let defined = self.defined_ctes()?;
        let mut used = HashSet::new();
        collect_source_ctes(&self.source, &mut used);
        if let Some(references) = references {
            for reference in references.values() {
                if defined.contains(reference.table_name) {
                    used.insert(reference.table_name.to_string());
                }
            }
        }
        for node in self.expression_nodes() {
            collect_expr_ctes(node, &mut used);
        }
        validate_cte_usage(&defined, &used)
    }

    fn defined_ctes(&self) -> Result<HashSet<String>, QueryError> {
        let mut defined = HashSet::new();
        for cte in &self.ctes {
            let name = cte.data.name.to_string();
            if !defined.insert(name.clone()) {
                return Err(QueryError::BindError(format!("duplicate CTE '{}'", name)));
            }
        }
        Ok(defined)
    }

    fn expression_nodes(&self) -> impl Iterator<Item = &ExprNode> {
        self.filters
            .iter()
            .map(|predicate| &predicate.node)
            .chain(self.groups.iter())
            .chain(self.having.iter().map(|predicate| &predicate.node))
            .chain(self.orders.iter().map(|order| &order.expr))
            .chain(
                self.output_assignments
                    .iter()
                    .map(|assignment| &assignment.expr),
            )
    }

    fn collect_binds(&self) -> Result<HashMap<VarId, ArgValue>, QueryError> {
        let mut values = HashMap::new();
        self.collect_binds_into(&mut values)?;
        Ok(values)
    }

    pub(super) fn collect_binds_into(
        &self,
        values: &mut HashMap<VarId, ArgValue>,
    ) -> Result<(), QueryError> {
        for cte in &self.ctes {
            cte.data.scope.collect_binds_into(values)?;
        }
        for node in self.expression_nodes() {
            collect_expr_binds(node, values)?;
        }
        for (id, value) in &self.binds {
            insert_bind(values, *id, value.clone())?;
        }
        Ok(())
    }

    fn validate_insert_scope(&self) -> Result<(), QueryError> {
        if !self.filters.is_empty()
            || !self.groups.is_empty()
            || !self.having.is_empty()
            || !self.orders.is_empty()
            || !self.output_assignments.is_empty()
            || !self.ctes.is_empty()
        {
            return Err(QueryError::BindError(
                "insert does not support query-scope modifiers".to_string(),
            ));
        }
        Ok(())
    }

    fn validate_insert_returning_scope(&self) -> Result<(), QueryError> {
        if !self.filters.is_empty()
            || !self.groups.is_empty()
            || !self.having.is_empty()
            || !self.orders.is_empty()
            || !self.ctes.is_empty()
        {
            return Err(QueryError::BindError(
                "insert returning does not support query-scope modifiers".to_string(),
            ));
        }
        Ok(())
    }

    fn validate_update_scope(&self) -> Result<(), QueryError> {
        if !self.groups.is_empty()
            || !self.having.is_empty()
            || !self.orders.is_empty()
            || !self.output_assignments.is_empty()
        {
            return Err(QueryError::BindError(
                "update does not support select-only modifiers".to_string(),
            ));
        }
        self.validate_mutation_filters()
    }

    fn validate_update_returning_scope(&self) -> Result<(), QueryError> {
        if !self.groups.is_empty() || !self.having.is_empty() || !self.orders.is_empty() {
            return Err(QueryError::BindError(
                "update returning does not support select-only modifiers".to_string(),
            ));
        }
        self.validate_mutation_filters()
    }
}
