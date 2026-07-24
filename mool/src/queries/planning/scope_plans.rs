//! Query planning and validation for typed query scopes.

use indexmap::IndexMap;
use std::any::{TypeId, type_name};
use std::collections::{HashMap, HashSet};

use crate::argvalue::ArgValue;
use crate::commons::Arguments;
use crate::interfaces::{Model, Record};
use crate::placeholders::Dialect;
use crate::relations::ReferenceMeta;

use super::super::batch::BatchInsertMode;
use super::super::batch_validation::{
    resolve_batch_insert_mode, validate_batch_update_columns, validate_batch_update_keys,
    validate_model_table, validate_unique_columns,
};
use super::super::binds::{
    bind_insert_rows, bind_update_rows, collect_expr_binds, collect_expr_ctes, collect_source_ctes,
    finish_plan, insert_bind, insert_columns, validate_bind_columns, validate_cte_usage,
    validate_output_assignments,
};
use super::super::expr::ExprNode;
use super::super::handles::VarId;
use super::super::plan::QueryPlan;
use super::super::render::{Renderer, SelectModel};
use super::super::scope::QueryScope;
use super::super::set::SetOp;
use super::super::source::{CteSource, SelectSource, Source};
use super::super::validate::{
    output_columns, reject_window, source_table, validate_conflict_columns, validate_expr_owners,
    validate_identifier,
};
use super::super::values::WriteInput;
use crate::QueryError;

impl QueryScope {
    pub(in crate::queries) fn plan_all<T>(&self, dialect: Dialect) -> Result<QueryPlan, QueryError>
    where
        T: Record + 'static,
    {
        self.plan_select::<T>(None, dialect)
    }

    pub(in crate::queries) fn plan_first<T>(
        &self,
        dialect: Dialect,
    ) -> Result<QueryPlan, QueryError>
    where
        T: Record + 'static,
    {
        self.plan_select::<T>(Some((0, 1)), dialect)
    }

    pub(in crate::queries) fn plan_one<T>(&self, dialect: Dialect) -> Result<QueryPlan, QueryError>
    where
        T: Record + 'static,
    {
        self.plan_select::<T>(Some((0, 2)), dialect)
    }

    pub(in crate::queries) fn plan_slice<T>(
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

    pub(in crate::queries) fn plan_insert<W>(
        &self,
        row: &W,
        dialect: Dialect,
    ) -> Result<QueryPlan, QueryError>
    where
        W: WriteInput,
    {
        self.validate_scope_errors()?;
        self.validate_insert_scope()?;
        let parts = row.insert_shape(source_table(&self.source)?)?;
        let mut renderer = Renderer::with_prebound(dialect, parts.prebound_count);
        let sql = renderer.render_insert(self, &parts, false, &[], None)?;
        finish_plan(renderer.plan(sql, None, self.collect_binds()?))
    }

    pub(in crate::queries) fn plan_update<W>(
        &self,
        row: &W,
        dialect: Dialect,
    ) -> Result<QueryPlan, QueryError>
    where
        W: WriteInput,
    {
        self.validate_scope_errors()?;
        self.validate_update_scope()?;
        let parts = row.update_shape(source_table(&self.source)?)?;
        let mut renderer = Renderer::with_prebound(dialect, parts.prebound_count);
        let sql = renderer.render_update(self, &parts, None)?;
        finish_plan(renderer.plan(sql, None, self.collect_binds()?))
    }

    pub(in crate::queries) fn plan_delete(
        &self,
        dialect: Dialect,
    ) -> Result<QueryPlan, QueryError> {
        self.validate_scope_errors()?;
        self.validate_mutation_filters()?;
        let mut renderer = Renderer::new(dialect);
        let sql = renderer.render_delete(self, None)?;
        finish_plan(renderer.plan(sql, None, self.collect_binds()?))
    }

    pub(in crate::queries) fn plan_count(&self, dialect: Dialect) -> Result<QueryPlan, QueryError> {
        self.validate_scope_errors()?;
        let model = SelectModel::source_only(&self.source)?;
        self.validate_aggregate(&model, None)?;
        let mut renderer = Renderer::new(dialect);
        let sql = renderer.render_count(self, &model)?;
        finish_plan(renderer.plan(sql, None, self.collect_binds()?))
    }

    pub(in crate::queries) fn plan_exists(
        &self,
        dialect: Dialect,
    ) -> Result<QueryPlan, QueryError> {
        self.validate_scope_errors()?;
        let model = SelectModel::source_only(&self.source)?;
        self.validate_aggregate(&model, None)?;
        let mut renderer = Renderer::new(dialect);
        let sql = renderer.render_exists(self, &model)?;
        finish_plan(renderer.plan(sql, None, self.collect_binds()?))
    }

    pub(in crate::queries) fn plan_scalar(
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

    pub(in crate::queries) fn plan_set_all<T>(
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

    pub(in crate::queries) fn select_source<T>(
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

    pub(in crate::queries) fn compose_select_source<T>(
        &mut self,
        name: &str,
        slice: Option<(usize, usize)>,
    ) -> SelectSource
    where
        T: Record + 'static,
    {
        match self.select_source::<T>(name, slice) {
            Ok(source) => source,
            Err(error) => {
                self.errors.push(error);
                let model = SelectModel::deferred::<T>(&self.source);
                let columns = model
                    .columns
                    .iter()
                    .map(|column| column.rsplit('.').next().unwrap_or(column).to_string())
                    .collect();
                SelectSource {
                    model,
                    slice,
                    columns,
                }
            }
        }
    }

    pub(in crate::queries) fn plan_insert_with_args<W>(
        &self,
        row: &W,
        dialect: Dialect,
    ) -> Result<(QueryPlan, Arguments<'static>), QueryError>
    where
        W: WriteInput,
    {
        self.validate_scope_errors()?;
        self.validate_insert_scope()?;
        let parts = row.insert_parts(source_table(&self.source)?)?;
        let mut renderer = Renderer::with_prebound(dialect, parts.prebound_count);
        let sql = renderer.render_insert(self, &parts, false, &[], None)?;
        Ok((
            finish_plan(renderer.plan(sql, None, self.collect_binds()?))?,
            parts.args,
        ))
    }

    pub(in crate::queries) fn plan_update_with_args<W>(
        &self,
        row: &W,
        dialect: Dialect,
    ) -> Result<(QueryPlan, Arguments<'static>), QueryError>
    where
        W: WriteInput,
    {
        self.validate_scope_errors()?;
        self.validate_update_scope()?;
        let parts = row.update_parts(source_table(&self.source)?)?;
        let mut renderer = Renderer::with_prebound(dialect, parts.prebound_count);
        let sql = renderer.render_update(self, &parts, None)?;
        Ok((
            finish_plan(renderer.plan(sql, None, self.collect_binds()?))?,
            parts.args,
        ))
    }

    /// Plans one batch-insert statement with its conflict and returning policy.
    pub(in crate::queries) fn plan_batch_insert_mode_with_args<T>(
        &self,
        rows: &[T],
        dialect: Dialect,
        mode: &BatchInsertMode,
        returning: Option<&SelectModel>,
    ) -> Result<(QueryPlan, Arguments<'static>), QueryError>
    where
        T: Record + 'static,
    {
        let plan = self.plan_batch_insert_mode::<T>(rows.len(), dialect, mode, returning)?;
        let args = bind_insert_rows(rows, T::record_insert_column_names().len())?;
        Ok((plan, args))
    }

    pub(in crate::queries) fn plan_batch_insert_mode<T>(
        &self,
        row_count: usize,
        dialect: Dialect,
        mode: &BatchInsertMode,
        returning: Option<&SelectModel>,
    ) -> Result<QueryPlan, QueryError>
    where
        T: Record + 'static,
    {
        self.validate_scope_errors()?;
        self.validate_insert_scope()?;
        let table = source_table(&self.source)?;
        let columns = insert_columns::<T>()?;
        validate_bind_columns(table, &columns)?;
        let mode = resolve_batch_insert_mode::<T>(mode, table, &columns)?;
        let prebound_count =
            row_count
                .checked_mul(columns.len())
                .ok_or(QueryError::BatchParameterOverflow {
                    rows: row_count,
                    columns: columns.len(),
                })?;
        let mut renderer = Renderer::with_prebound(dialect, prebound_count);
        let sql = renderer.render_batch_insert(self, &columns, row_count, &mode, returning)?;
        let plan = match returning {
            Some(model) => self.finish_returning(renderer, sql, model)?,
            None => finish_plan(renderer.plan(sql, None, self.collect_binds()?))?,
        };
        Ok(plan)
    }

    pub(in crate::queries) fn plan_batch_update_with_args<T>(
        &self,
        rows: &[T],
        update_columns: &[super::super::expr::ColumnRef],
        dialect: Dialect,
        returning: Option<&SelectModel>,
    ) -> Result<(QueryPlan, Arguments<'static>), QueryError>
    where
        T: Model + 'static,
    {
        let plan = self.plan_batch_update::<T>(rows, update_columns, dialect, returning)?;
        let primary_keys = T::primary_key_columns();
        let mut bind_names = primary_keys.to_vec();
        bind_names.extend(update_columns.iter().map(|column| column.name.as_ref()));
        let args = bind_update_rows(rows, &bind_names)?;
        Ok((plan, args))
    }

    pub(in crate::queries) fn plan_batch_update<T>(
        &self,
        rows: &[T],
        update_columns: &[super::super::expr::ColumnRef],
        dialect: Dialect,
        returning: Option<&SelectModel>,
    ) -> Result<QueryPlan, QueryError>
    where
        T: Model + 'static,
    {
        self.validate_batch_update_input::<T>(rows, update_columns)?;
        let primary_keys = T::primary_key_columns();
        let width = primary_keys.len() + update_columns.len();
        let prebound_count =
            rows.len()
                .checked_mul(width)
                .ok_or(QueryError::BatchParameterOverflow {
                    rows: rows.len(),
                    columns: width,
                })?;
        let mut renderer = Renderer::with_prebound(dialect, prebound_count);
        let update_names = update_columns
            .iter()
            .map(|column| column.name.as_ref())
            .collect::<Vec<_>>();
        let sql = renderer.render_batch_update(
            self,
            primary_keys,
            &update_names,
            rows.len(),
            returning,
        )?;
        let plan = match returning {
            Some(model) => self.finish_returning(renderer, sql, model)?,
            None => finish_plan(renderer.plan(sql, None, self.collect_binds()?))?,
        };
        Ok(plan)
    }

    pub(in crate::queries) fn validate_batch_update_input<T>(
        &self,
        rows: &[T],
        update_columns: &[super::super::expr::ColumnRef],
    ) -> Result<(), QueryError>
    where
        T: Model + 'static,
    {
        self.validate_scope_errors()?;
        self.validate_batch_update_scope()?;
        let table = source_table(&self.source)?;
        validate_model_table::<T>(table)?;
        validate_unique_columns(update_columns, "batch update fields")?;
        validate_conflict_columns(update_columns, table)?;
        validate_batch_update_columns::<T>(update_columns, T::primary_key_columns())?;
        validate_batch_update_keys(rows)
    }

    #[cfg(feature = "postgres")]
    pub(in crate::queries) fn plan_batch_unnest_with_args<T>(
        &self,
        rows: &[T],
        dialect: Dialect,
        mode: &BatchInsertMode,
        returning: Option<&SelectModel>,
    ) -> Result<(QueryPlan, Arguments<'static>), QueryError>
    where
        T: crate::BatchRecord + 'static,
        T::BatchColumns: crate::backend::PgBatchColumns,
    {
        use crate::backend::PgBatchColumns as _;
        let plan = self.plan_batch_unnest::<T>(rows.len(), dialect, mode, returning)?;
        let batch_columns =
            T::batch_columns(rows).map_err(|error| QueryError::BindError(error.to_string()))?;
        let row_count = batch_columns.row_count()?;
        if row_count == 0 {
            return Err(QueryError::EmptyBatch {
                operation: "PostgreSQL UNNEST insert",
            });
        }
        let column_count = batch_columns.column_count();
        let expected_columns = T::record_insert_column_names().len();
        if column_count != expected_columns {
            return Err(QueryError::BindCountMismatch {
                expected: expected_columns,
                got: column_count,
            });
        }
        let mut args = Arguments::default();
        batch_columns.bind(&mut args)?;
        Ok((plan, args))
    }

    #[cfg(feature = "postgres")]
    pub(in crate::queries) fn plan_batch_unnest<T>(
        &self,
        row_count: usize,
        dialect: Dialect,
        mode: &BatchInsertMode,
        returning: Option<&SelectModel>,
    ) -> Result<QueryPlan, QueryError>
    where
        T: crate::BatchRecord + 'static,
        T::BatchColumns: crate::backend::PgBatchColumns,
    {
        if row_count == 0 {
            return Err(QueryError::EmptyBatch {
                operation: "PostgreSQL UNNEST insert",
            });
        }
        self.validate_scope_errors()?;
        self.validate_insert_scope()?;
        let table = source_table(&self.source)?;
        let columns = insert_columns::<T>()?;
        validate_bind_columns(table, &columns)?;
        let mode = resolve_batch_insert_mode::<T>(mode, table, &columns)?;
        let mut renderer = Renderer::with_prebound(dialect, columns.len());
        let sql = renderer.render_batch_unnest(self, &columns, &mode, returning)?;
        match returning {
            Some(model) => self.finish_returning(renderer, sql, model),
            None => finish_plan(renderer.plan(sql, None, self.collect_binds()?)),
        }
    }

    pub(in crate::queries) fn plan_insert_returning<W>(
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
        let parts = row.insert_parts(source_table(&self.source)?)?;
        let mut renderer = Renderer::with_prebound(dialect, parts.prebound_count);
        let sql = renderer.render_insert(self, &parts, false, &[], Some(returning))?;
        Ok((self.finish_returning(renderer, sql, returning)?, parts.args))
    }

    pub(in crate::queries) fn plan_insert_returning_shape<W>(
        &self,
        row: &W,
        dialect: Dialect,
        returning: &SelectModel,
    ) -> Result<QueryPlan, QueryError>
    where
        W: WriteInput,
    {
        self.validate_scope_errors()?;
        self.validate_insert_returning_scope()?;
        let parts = row.insert_shape(source_table(&self.source)?)?;
        let mut renderer = Renderer::with_prebound(dialect, parts.prebound_count);
        let sql = renderer.render_insert(self, &parts, false, &[], Some(returning))?;
        self.finish_returning(renderer, sql, returning)
    }

    pub(in crate::queries) fn plan_update_returning<W>(
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
        let parts = row.update_parts(source_table(&self.source)?)?;
        let mut renderer = Renderer::with_prebound(dialect, parts.prebound_count);
        let sql = renderer.render_update(self, &parts, Some(returning))?;
        Ok((self.finish_returning(renderer, sql, returning)?, parts.args))
    }

    pub(in crate::queries) fn plan_update_returning_shape<W>(
        &self,
        row: &W,
        dialect: Dialect,
        returning: &SelectModel,
    ) -> Result<QueryPlan, QueryError>
    where
        W: WriteInput,
    {
        self.validate_scope_errors()?;
        self.validate_update_returning_scope()?;
        let parts = row.update_shape(source_table(&self.source)?)?;
        let mut renderer = Renderer::with_prebound(dialect, parts.prebound_count);
        let sql = renderer.render_update(self, &parts, Some(returning))?;
        self.finish_returning(renderer, sql, returning)
    }

    pub(in crate::queries) fn plan_delete_returning(
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
        match &self.source {
            Source::Cte(cte) => cte.data.scope.validate_scope_errors()?,
            Source::Subquery(subquery) => subquery.data.scope.validate_scope_errors()?,
            Source::Table(_) => {}
        }
        for cte in &self.ctes {
            cte.data.scope.validate_scope_errors()?;
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
        self.reject_row_only_modifiers("aggregate terminals")?;
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
        self.reject_row_only_modifiers("mutation terminals")?;
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
        for expr in &self.distinct_on {
            reject_window(expr, "DISTINCT ON")?;
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
        let mut named_used = HashSet::new();
        collect_source_ctes(&self.source, &mut used);
        if let Some(references) = references {
            for reference in references.values() {
                if defined.contains_key(reference.table_name) {
                    named_used.insert(reference.table_name.to_string());
                }
            }
        }
        for node in self.expression_nodes() {
            collect_expr_ctes(node, &mut used);
        }
        validate_cte_usage(&defined, &used, &named_used)
    }

    fn defined_ctes(&self) -> Result<IndexMap<String, CteSource>, QueryError> {
        let mut defined = IndexMap::new();
        for cte in &self.ctes {
            let name = cte.data.name.to_string();
            if defined.contains_key(&name) {
                return Err(QueryError::BindError(format!("duplicate CTE '{}'", name)));
            }
            defined.insert(name, cte.clone());
        }
        Ok(defined)
    }

    fn expression_nodes(&self) -> impl Iterator<Item = &ExprNode> {
        self.filters
            .iter()
            .map(|predicate| &predicate.node)
            .chain(self.groups.iter())
            .chain(self.distinct_on.iter())
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

    pub(in crate::queries) fn collect_binds_into(
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
        if self.distinct
            || !self.distinct_on.is_empty()
            || !self.filters.is_empty()
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
        if self.distinct
            || !self.distinct_on.is_empty()
            || !self.filters.is_empty()
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
        if self.distinct
            || !self.distinct_on.is_empty()
            || !self.groups.is_empty()
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
        if self.distinct
            || !self.distinct_on.is_empty()
            || !self.groups.is_empty()
            || !self.having.is_empty()
            || !self.orders.is_empty()
        {
            return Err(QueryError::BindError(
                "update returning does not support select-only modifiers".to_string(),
            ));
        }
        self.validate_mutation_filters()
    }

    fn validate_batch_update_scope(&self) -> Result<(), QueryError> {
        if self.distinct
            || !self.distinct_on.is_empty()
            || !self.groups.is_empty()
            || !self.having.is_empty()
            || !self.orders.is_empty()
        {
            return Err(QueryError::BindError(
                "batch update does not support select-only modifiers".to_string(),
            ));
        }
        if !self.ctes.is_empty() {
            return Err(QueryError::BindError(
                "batch update does not support CTEs".to_string(),
            ));
        }
        self.validate_ctes(None)?;
        for node in self.expression_nodes() {
            reject_window(node, "batch update statements")?;
            validate_expr_owners(node, &self.source, None, None, false)?;
        }
        Ok(())
    }

    fn reject_row_only_modifiers(&self, terminal: &'static str) -> Result<(), QueryError> {
        if self.distinct {
            return Err(QueryError::InvalidModifier {
                modifier: "distinct",
                terminal,
            });
        }
        if !self.distinct_on.is_empty() {
            return Err(QueryError::InvalidModifier {
                modifier: "distinct on",
                terminal,
            });
        }
        #[cfg(any(feature = "postgres", feature = "mysql", feature = "mariadb"))]
        if self.lock.is_some() {
            return Err(QueryError::InvalidModifier {
                modifier: "row lock",
                terminal,
            });
        }
        Ok(())
    }
}
