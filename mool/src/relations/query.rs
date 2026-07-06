//! Typed relation predicate and aggregate helpers.

use std::marker::PhantomData;

use crate::interfaces::Model;
use crate::queries::{
    __private::HasCols, Expr, IntoExpr, ModelTable, Predicate, many_to_many_exists,
    relation_aggregate, relation_exists,
};

use super::{Backref, ManyBackref, ManyToMany};

/// Predicate helper for a reverse relation.
#[doc(hidden)]
pub struct BackrefRef<R>
where
    R: Backref,
{
    _marker: PhantomData<fn() -> R>,
}

/// Predicate helper for a many-to-many relation.
#[doc(hidden)]
pub struct ManyToManyRef<R>
where
    R: ManyToMany,
{
    _marker: PhantomData<fn() -> R>,
}

impl<R> BackrefRef<R>
where
    R: Backref,
    R::From: HasCols,
{
    pub(crate) fn new(_source: &ModelTable<R::From>) -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<R> ManyToManyRef<R>
where
    R: ManyToMany,
    R::From: HasCols,
{
    pub(crate) fn new(_source: &ModelTable<R::From>) -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<R> BackrefRef<R>
where
    R: ManyBackref,
    R::To: HasCols,
{
    /// Matches parent rows with at least one related child row.
    pub fn exists(self) -> Predicate {
        let target = R::To::table();
        relation_exists(R::meta(), target.table_source(), None, false)
    }

    /// Matches parent rows with at least one related child satisfying `f`.
    pub fn any<F>(self, f: F) -> Predicate
    where
        F: FnOnce(&ModelTable<R::To>) -> Predicate,
    {
        let target = R::To::table();
        relation_exists(R::meta(), target.table_source(), Some(f(&target)), false)
    }

    /// Matches parent rows with no related child rows.
    pub fn none(self) -> Predicate {
        let target = R::To::table();
        relation_exists(R::meta(), target.table_source(), None, true)
    }

    /// Counts related child rows.
    pub fn count(self) -> Expr<i64> {
        let target = R::To::table();
        relation_aggregate("COUNT", R::meta(), target.table_source(), None::<Expr<i64>>)
    }

    /// Sums a related child expression.
    pub fn sum<T, F, E>(self, f: F) -> Expr<T>
    where
        F: FnOnce(&ModelTable<R::To>) -> E,
        E: IntoExpr<T>,
        T: 'static,
    {
        self.aggregate("SUM", Some(f))
    }

    /// Averages a related child expression.
    pub fn avg<T, F, E>(self, f: F) -> Expr<f64>
    where
        F: FnOnce(&ModelTable<R::To>) -> E,
        E: IntoExpr<T>,
        T: 'static,
    {
        let target = R::To::table();
        relation_aggregate(
            "AVG",
            R::meta(),
            target.table_source(),
            Some(f(&target).into_expr()),
        )
    }

    /// Minimum of a related child expression.
    pub fn min<T, F, E>(self, f: F) -> Expr<T>
    where
        F: FnOnce(&ModelTable<R::To>) -> E,
        E: IntoExpr<T>,
        T: 'static,
    {
        self.aggregate("MIN", Some(f))
    }

    /// Maximum of a related child expression.
    pub fn max<T, F, E>(self, f: F) -> Expr<T>
    where
        F: FnOnce(&ModelTable<R::To>) -> E,
        E: IntoExpr<T>,
        T: 'static,
    {
        self.aggregate("MAX", Some(f))
    }

    fn aggregate<T, F, E>(self, function: &'static str, f: Option<F>) -> Expr<T>
    where
        F: FnOnce(&ModelTable<R::To>) -> E,
        E: IntoExpr<T>,
        T: 'static,
    {
        let target = R::To::table();
        let expr = f.map(|f| f(&target).into_expr());
        relation_aggregate(function, R::meta(), target.table_source(), expr)
    }
}

impl<R> ManyToManyRef<R>
where
    R: ManyToMany,
    R::Through: HasCols,
    R::To: HasCols,
{
    /// Matches parent rows with at least one related row satisfying `f`.
    pub fn any<F>(self, f: F) -> Predicate
    where
        F: FnOnce(&ModelTable<R::To>) -> Predicate,
    {
        let target = R::To::table();
        let through = R::Through::table();
        many_to_many_exists(
            R::from_through(),
            R::through_to(),
            through.table_source(),
            target.table_source(),
            Some(f(&target)),
            false,
        )
    }

    /// Matches parent rows with no related rows.
    pub fn none(self) -> Predicate {
        let target = R::To::table();
        let through = R::Through::table();
        many_to_many_exists(
            R::from_through(),
            R::through_to(),
            through.table_source(),
            target.table_source(),
            None,
            true,
        )
    }
}
