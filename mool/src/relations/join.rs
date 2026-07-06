//! Trait-first complex join support for typed projections.

use crate::queries::__private::HasCols;
use crate::queries::{ModelTable, Predicate};

use super::reference::JoinType;

/// Typed context passed to custom join predicates.
pub struct JoinCtx<From, To>
where
    From: crate::Model + HasCols,
    To: crate::Model + HasCols,
{
    /// Source model table columns.
    pub from: ModelTable<From>,
    /// Joined model table columns.
    pub to: ModelTable<To>,
}

/// Custom typed join predicate for complex `ON` clauses.
pub trait JoinRelation {
    /// Source model.
    type From: crate::Model + HasCols;
    /// Joined model.
    type To: crate::Model + HasCols;

    /// SQL join type.
    const JOIN_TYPE: JoinType;

    /// Builds the typed `ON` predicate.
    fn on(ctx: JoinCtx<Self::From, Self::To>) -> Predicate;
}
