//! Query scope composition and returning-scope helpers.
use std::any::{TypeId, type_name};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;

use crate::argvalue::ArgValue;
use crate::commons::Arguments;
use crate::filters::{FilterBuilder, Filterable};
use crate::interfaces::Record;
use crate::placeholders::Dialect;

use super::binds::validate_output_assignments;
use super::expr::{ExprNode, IntoExpr, OrderExpr, Predicate};
use super::handles::{Var, VarId};
use super::output::{IntoOutputTarget, ReturningUsing, SelectAssignment, select_assignment};
use super::plan::QueryPlan;
use super::render::SelectModel;
use super::source::{Cte, CteData, CteSource, Source, Subquery, SubqueryData, SubquerySource};
use super::traits::{IntoColumnRef, Projectable};
use super::validate::{
    generated_source_name, validate_returning_projection, validate_returning_supported,
};
use super::values::WriteInput;
use crate::QueryError;

/// Composable query scope rooted at one table.
#[derive(Clone)]
pub struct QueryScope {
    pub(super) source: Source,
    pub(super) ctes: Vec<CteSource>,
    pub(super) filters: Vec<Predicate>,
    pub(super) groups: Vec<ExprNode>,
    pub(super) having: Vec<Predicate>,
    pub(super) orders: Vec<OrderExpr>,
    pub(super) output_assignments: Vec<SelectAssignment>,
    pub(super) binds: HashMap<VarId, ArgValue>,
    pub(super) errors: Vec<QueryError>,
}

/// Write scope that returns rows through a `RETURNING` projection.
pub struct ReturningScope<R> {
    pub(super) scope: QueryScope,
    pub(super) _marker: PhantomData<fn() -> R>,
}

impl QueryScope {
    pub(super) fn new(source: Source) -> Self {
        Self {
            source,
            ctes: Vec::new(),
            filters: Vec::new(),
            groups: Vec::new(),
            having: Vec::new(),
            orders: Vec::new(),
            output_assignments: Vec::new(),
            binds: HashMap::new(),
            errors: Vec::new(),
        }
    }

    /// Adds a WHERE predicate.
    pub fn filter(mut self, predicate: Predicate) -> Self {
        self.filters.push(predicate);
        self
    }

    /// Applies a typed, model-bound filter to this query's WHERE clause.
    pub fn filter_with<F>(mut self, filter: &F) -> Self
    where
        F: Filterable,
    {
        let builder = FilterBuilder::new(<F::Model as crate::Model>::table());
        self.filters
            .extend(filter.apply_filter(builder).into_predicates());
        self
    }

    /// Adds a GROUP BY expression.
    pub fn group_by<T>(mut self, expr: impl IntoExpr<T>) -> Self {
        self.groups.push(expr.into_expr().node);
        self
    }

    /// Adds a HAVING predicate.
    pub fn having(mut self, predicate: Predicate) -> Self {
        self.having.push(predicate);
        self
    }

    /// Adds an ORDER BY expression.
    pub fn order_by(mut self, expr: OrderExpr) -> Self {
        self.orders.push(expr);
        self
    }

    /// Binds a runtime value for a `var(...)`.
    pub fn bind<T>(mut self, var: &Var<T>, value: T) -> Self
    where
        T: Clone
            + for<'q> sqlx::Encode<'q, crate::commons::Database>
            + sqlx::Type<crate::commons::Database>
            + Send
            + Sync
            + 'static,
    {
        match self.binds.entry(var.data.id) {
            std::collections::hash_map::Entry::Occupied(_) => {
                let name = var.name().unwrap_or("anonymous var");
                self.errors.push(QueryError::BindError(format!(
                    "duplicate binding for '{}'",
                    name
                )));
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(ArgValue::new(value));
            }
        }
        self
    }

    /// Adds a CTE definition to this query scope.
    pub fn with<T>(mut self, cte: &Cte<T>) -> Self
    where
        T: Projectable,
    {
        self.ctes.push(cte.as_source());
        self
    }

    pub(super) fn cte<T>(self) -> Result<Cte<T>, QueryError>
    where
        T: Record + Projectable + 'static,
    {
        self.cte_as::<T>(&generated_source_name::<T>("cte"))
    }

    pub(super) fn cte_as<T>(self, name: &str) -> Result<Cte<T>, QueryError>
    where
        T: Record + Projectable + 'static,
    {
        self.cte_as_slice::<T>(name, None)
    }

    pub(super) fn cte_as_slice<T>(
        self,
        name: &str,
        slice: Option<(usize, usize)>,
    ) -> Result<Cte<T>, QueryError>
    where
        T: Record + Projectable + 'static,
    {
        let source = self.select_source::<T>(name, slice)?;
        let data = Arc::new(CteData {
            name: Arc::from(name),
            scope: self,
            model: source.model,
            slice: source.slice,
            columns: source.columns,
        });
        let cte_source = CteSource { data: data.clone() };
        let columns = T::projected_columns(super::source::ProjectionSource::new(Source::Cte(
            cte_source,
        )));
        Ok(Cte {
            data,
            columns,
            _marker: PhantomData,
        })
    }

    pub(super) fn subquery<T>(self) -> Result<Subquery<T>, QueryError>
    where
        T: Record + Projectable + 'static,
    {
        self.subquery_as::<T>(&generated_source_name::<T>("subquery"))
    }

    pub(super) fn subquery_as<T>(self, name: &str) -> Result<Subquery<T>, QueryError>
    where
        T: Record + Projectable + 'static,
    {
        self.subquery_as_slice::<T>(name, None)
    }

    pub(super) fn subquery_as_slice<T>(
        self,
        name: &str,
        slice: Option<(usize, usize)>,
    ) -> Result<Subquery<T>, QueryError>
    where
        T: Record + Projectable + 'static,
    {
        let source = self.select_source::<T>(name, slice)?;
        let data = Arc::new(SubqueryData {
            name: Arc::from(name),
            scope: self,
            model: source.model,
            slice: source.slice,
            columns: source.columns,
        });
        let subquery_source = SubquerySource { data: data.clone() };
        let columns = T::projected_columns(super::source::ProjectionSource::new(Source::Subquery(
            subquery_source,
        )));
        Ok(Subquery {
            data,
            columns,
            _marker: PhantomData,
        })
    }

    /// Uses a record projection as the `RETURNING` shape for write terminals.
    pub fn returning<R>(self) -> ReturningScope<R>
    where
        R: Record,
    {
        ReturningScope {
            scope: self,
            _marker: PhantomData,
        }
    }
}

impl<R> ReturningScope<R>
where
    R: Record + 'static,
{
    fn model(&self, dialect: Dialect) -> Result<SelectModel, QueryError> {
        validate_returning_supported(dialect)?;
        let model = SelectModel::new::<R>(&self.scope.source)?;
        validate_returning_projection(&model)?;
        validate_output_assignments(
            &self.scope.output_assignments,
            &model.columns,
            TypeId::of::<R>(),
            type_name::<R>(),
        )?;
        Ok(model)
    }

    /// Adds computed expressions to the `RETURNING` projection.
    pub fn set<V>(mut self, target: impl IntoOutputTarget<V>, expr: impl IntoExpr<V>) -> Self
    where
        R: super::output::HasOutputCols,
    {
        self.scope
            .output_assignments
            .push(select_assignment(target, expr));
        self
    }

    /// Adds computed expressions to the `RETURNING` projection.
    #[doc(hidden)]
    pub fn using<F>(mut self, f: F) -> Self
    where
        R: super::output::HasOutputCols,
        F: FnOnce(ReturningUsing<R>) -> ReturningUsing<R>,
    {
        self.scope.output_assignments = f(ReturningUsing::new()).into_selects().assignments;
        self
    }

    pub(super) fn plan_insert<W>(
        &self,
        row: &W,
        dialect: Dialect,
    ) -> Result<(QueryPlan, Arguments<'static>), QueryError>
    where
        W: WriteInput,
    {
        let model = self.model(dialect)?;
        self.scope.plan_insert_returning(row, dialect, &model)
    }

    pub(super) fn plan_update<W>(
        &self,
        row: &W,
        dialect: Dialect,
    ) -> Result<(QueryPlan, Arguments<'static>), QueryError>
    where
        W: WriteInput,
    {
        let model = self.model(dialect)?;
        self.scope.plan_update_returning(row, dialect, &model)
    }

    pub(super) fn plan_delete(&self, dialect: Dialect) -> Result<QueryPlan, QueryError> {
        let model = self.model(dialect)?;
        self.scope.plan_delete_returning(dialect, &model)
    }

    pub(super) fn plan_batch_insert<T>(
        &self,
        rows: &[T],
        dialect: Dialect,
    ) -> Result<(QueryPlan, Arguments<'static>), QueryError>
    where
        T: Record + 'static,
    {
        let model = self.model(dialect)?;
        self.scope
            .plan_batch_insert_returning(rows, dialect, &model)
    }

    pub(super) fn plan_batch_upsert<T, I, C>(
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
        let model = self.model(dialect)?;
        self.scope
            .plan_batch_upsert_returning(rows, conflict, dialect, &model)
    }
}
