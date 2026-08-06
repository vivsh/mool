//! Typed ordering builder used by `Sortable` implementations.

use std::ops::Deref;

use crate::interfaces::Model;
use crate::queries::__private::{HasCols, ModelTable};
use crate::queries::OrderExpr;

/// ORDER BY-only builder passed to typed sort implementations.
///
/// The builder dereferences to the model's source-owned columns so sort inputs
/// can construct typed ordering expressions without accessing query execution.
pub struct SortBuilder<M>
where
    M: Model + HasCols,
{
    table: ModelTable<M>,
    orders: Vec<OrderExpr>,
}

impl<M> SortBuilder<M>
where
    M: Model + HasCols,
{
    pub(crate) fn new(table: ModelTable<M>) -> Self {
        Self {
            table,
            orders: Vec::new(),
        }
    }

    /// Appends a typed ORDER BY expression.
    pub fn sort(mut self, order: OrderExpr) -> Self {
        self.orders.push(order);
        self
    }

    pub(crate) fn into_orders(self) -> Vec<OrderExpr> {
        self.orders
    }
}

impl<M> Deref for SortBuilder<M>
where
    M: Model + HasCols,
{
    type Target = <M as HasCols>::Columns;

    fn deref(&self) -> &Self::Target {
        &self.table
    }
}
