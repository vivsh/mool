//! Typed predicate builder used by `Filterable` implementations.

use std::ops::Deref;

use crate::interfaces::Model;
use crate::queries::__private::{Column, HasCols, ModelTable};
use crate::queries::{IntoExpr, Predicate};

/// WHERE-only builder passed to typed filter implementations.
///
/// The builder dereferences to the model's source-owned columns so filters can
/// build normal typed predicates without exposing query execution or mutation
/// APIs.
pub struct FilterBuilder<M>
where
    M: Model + HasCols,
{
    table: ModelTable<M>,
    predicates: Vec<Predicate>,
}

impl<M> FilterBuilder<M>
where
    M: Model + HasCols,
{
    pub(crate) fn new(table: ModelTable<M>) -> Self {
        Self {
            table,
            predicates: Vec::new(),
        }
    }

    /// Appends a typed WHERE predicate.
    pub fn filter(mut self, predicate: Predicate) -> Self {
        self.predicates.push(predicate);
        self
    }

    pub(crate) fn into_predicates(self) -> Vec<Predicate> {
        self.predicates
    }
}

impl<M> Deref for FilterBuilder<M>
where
    M: Model + HasCols,
{
    type Target = <M as HasCols>::Columns;

    fn deref(&self) -> &Self::Target {
        &self.table
    }
}

/// Builds a typed `IN (...)` predicate from a runtime value list.
///
/// This is hidden macro support for `#[derive(Filterable)]`. Empty lists should
/// be skipped by callers because SQL `IN ()` is invalid.
#[doc(hidden)]
pub fn in_values<T, I, V>(column: &Column<T>, values: I) -> Predicate
where
    I: IntoIterator<Item = V>,
    V: IntoExpr<T>,
{
    crate::queries::__private::in_list(column, values)
}
