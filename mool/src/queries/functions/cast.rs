//! Typed SQL cast expressions with a closed set of safe target types.

use std::marker::PhantomData;

use crate::QueryError;

use super::super::expr::{Expr, IntoExpr};
use super::super::extension::{DbExpression, ExprRenderCtx, FunctionArgs, custom};

mod sealed {
    pub trait Sealed {}

    impl Sealed for i64 {}
    impl Sealed for f64 {}
    impl Sealed for String {}
    impl Sealed for bool {}
}

/// Rust types that have a safe built-in SQL cast target for the selected backend.
pub trait CastTarget: sealed::Sealed + Send + Sync + 'static {
    #[doc(hidden)]
    const SQL_TYPE: &'static str;
}

impl CastTarget for i64 {
    #[cfg(feature = "sqlite")]
    const SQL_TYPE: &'static str = "INTEGER";
    #[cfg(any(feature = "postgres", feature = "mysql", feature = "mariadb"))]
    const SQL_TYPE: &'static str = "BIGINT";
}

impl CastTarget for f64 {
    #[cfg(feature = "postgres")]
    const SQL_TYPE: &'static str = "DOUBLE PRECISION";
    #[cfg(feature = "sqlite")]
    const SQL_TYPE: &'static str = "REAL";
    #[cfg(any(feature = "mysql", feature = "mariadb"))]
    const SQL_TYPE: &'static str = "DOUBLE";
}

impl CastTarget for String {
    #[cfg(any(feature = "postgres", feature = "sqlite"))]
    const SQL_TYPE: &'static str = "TEXT";
    #[cfg(any(feature = "mysql", feature = "mariadb"))]
    const SQL_TYPE: &'static str = "CHAR";
}

impl CastTarget for bool {
    const SQL_TYPE: &'static str = "BOOLEAN";
}

/// Casts a typed expression to a supported Rust and SQL target type.
pub fn cast<T, U>(expr: impl IntoExpr<T>) -> Expr<U>
where
    T: 'static,
    U: CastTarget,
{
    custom(CastExpression::<U> {
        args: FunctionArgs::new((expr.into_expr(),)),
        marker: PhantomData,
    })
}

struct CastExpression<T> {
    args: FunctionArgs,
    marker: PhantomData<fn() -> T>,
}

impl<T> Clone for CastExpression<T> {
    fn clone(&self) -> Self {
        Self {
            args: self.args.clone(),
            marker: PhantomData,
        }
    }
}

impl<T> DbExpression<T> for CastExpression<T>
where
    T: CastTarget,
{
    fn args(&self) -> FunctionArgs {
        self.args.clone()
    }

    fn render(&self, ctx: &mut ExprRenderCtx<'_>) -> Result<String, QueryError> {
        Ok(format!("CAST({} AS {})", ctx.arg(0)?, T::SQL_TYPE))
    }
}
