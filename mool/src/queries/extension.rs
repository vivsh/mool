//! Typed extension hooks for database functions and expressions.

use std::borrow::Cow;
use std::marker::PhantomData;
use std::sync::Arc;

use crate::SqlDialect;

use super::dialect;
use super::expr::{Expr, ExprNode};
use super::handles::{Column, Var};
use super::source::ProjectedColumn;
use crate::QueryError;

/// Typed SQL function for the selected backend.
///
/// Implement this for reusable functions such as `unaccent`, JSON helpers, or
/// project-specific database functions. The renderer validates the function
/// for the active dialect before writing SQL.
pub trait DbFunction<T>: Clone + Send + Sync + 'static {
    /// Returns the SQL function name for the selected dialect.
    fn name(&self, dialect: SqlDialect) -> Result<Cow<'static, str>, QueryError>;

    /// Validates the argument count and backend-specific constraints.
    fn validate(&self, _dialect: SqlDialect, _arity: usize) -> Result<(), QueryError> {
        Ok(())
    }

    /// Whether this function can be rendered with `OVER (...)`.
    fn supports_window(&self) -> bool {
        false
    }
}

/// Advanced hook for typed custom expressions.
///
/// Prefer [`DbFunction`] for ordinary function calls. Use this hook when a
/// dialect-specific expression cannot be represented as `name(arg, ...)`.
pub trait DbExpression<T>: Clone + Send + Sync + 'static {
    /// Returns child expressions used by this custom expression.
    fn args(&self) -> FunctionArgs {
        FunctionArgs::default()
    }

    /// Validates backend-specific constraints before rendering.
    fn validate(&self, _dialect: SqlDialect) -> Result<(), QueryError> {
        Ok(())
    }

    /// Writes this expression using dialect and structured child access.
    fn render(&self, ctx: &mut ExprRenderCtx<'_>) -> Result<(), QueryError>;
}

/// Rendering context passed to [`DbExpression`].
pub struct ExprRenderCtx<'a> {
    dialect: SqlDialect,
    sql: String,
    render_arg: &'a mut dyn FnMut(usize, &mut String) -> Result<(), QueryError>,
}

impl<'a> ExprRenderCtx<'a> {
    pub(super) fn new(
        dialect: SqlDialect,
        render_arg: &'a mut dyn FnMut(usize, &mut String) -> Result<(), QueryError>,
    ) -> Self {
        Self {
            dialect,
            sql: String::new(),
            render_arg,
        }
    }

    /// Returns the dialect currently being rendered.
    pub fn dialect(&self) -> SqlDialect {
        self.dialect
    }

    /// Appends trusted SQL syntax owned by the expression implementation.
    pub fn push_sql(&mut self, sql: &str) {
        self.sql.push_str(sql);
    }

    /// Renders and appends one child expression at this exact occurrence.
    pub fn push_arg(&mut self, index: usize) -> Result<(), QueryError> {
        (self.render_arg)(index, &mut self.sql)
    }

    pub(super) fn finish(self) -> String {
        self.sql
    }
}

/// Type-erased function arguments captured by the typed AST.
#[derive(Clone, Default)]
pub struct FunctionArgs {
    pub(super) nodes: Vec<ExprNode>,
}

impl FunctionArgs {
    /// Builds expression arguments for custom database functions or expressions.
    pub fn new(args: impl IntoFunctionArgs) -> Self {
        args.into_function_args()
    }
}

/// Converts tuple arguments into typed function arguments.
pub trait IntoFunctionArgs {
    #[doc(hidden)]
    fn into_function_args(self) -> FunctionArgs;
}

/// Converts a single expression-like value into an untyped function argument.
#[doc(hidden)]
pub trait IntoAnyExpr {
    #[doc(hidden)]
    fn into_any_expr(self) -> FunctionArgs;
}

/// Creates a typed database function expression.
pub fn func<T, F, A>(function: F, args: A) -> Expr<T>
where
    T: 'static,
    F: DbFunction<T>,
    A: IntoFunctionArgs,
{
    let args = args.into_function_args();
    Expr::new(ExprNode::Function {
        function: Arc::new(FunctionAdapter::<T, F> {
            function,
            _marker: PhantomData,
        }),
        args: args.nodes,
    })
}

/// Creates a typed custom expression.
pub fn custom<T, E>(expression: E) -> Expr<T>
where
    T: 'static,
    E: DbExpression<T>,
{
    let args = expression.args();
    Expr::new(ExprNode::Custom {
        expression: Arc::new(CustomAdapter::<T, E> {
            expression,
            _marker: PhantomData,
        }),
        args: args.nodes,
    })
}

pub(super) trait FunctionSpec: Send + Sync {
    fn name(&self, dialect: SqlDialect) -> Result<Cow<'static, str>, QueryError>;

    fn validate(&self, dialect: SqlDialect, arity: usize) -> Result<(), QueryError>;

    fn supports_window(&self) -> bool;
}

pub(super) trait CustomExpressionSpec: Send + Sync {
    fn validate(&self, dialect: SqlDialect) -> Result<(), QueryError>;

    fn render(&self, ctx: &mut ExprRenderCtx<'_>) -> Result<(), QueryError>;
}

struct FunctionAdapter<T, F> {
    function: F,
    _marker: PhantomData<fn() -> T>,
}

struct CustomAdapter<T, E> {
    expression: E,
    _marker: PhantomData<fn() -> T>,
}

impl<T, F> FunctionSpec for FunctionAdapter<T, F>
where
    T: 'static,
    F: DbFunction<T>,
{
    fn name(&self, dialect: SqlDialect) -> Result<Cow<'static, str>, QueryError> {
        let name = self.function.name(dialect)?;
        dialect::render_function(dialect, name)
    }

    fn validate(&self, dialect: SqlDialect, arity: usize) -> Result<(), QueryError> {
        self.function.validate(dialect, arity)
    }

    fn supports_window(&self) -> bool {
        self.function.supports_window()
    }
}

impl<T, E> CustomExpressionSpec for CustomAdapter<T, E>
where
    T: 'static,
    E: DbExpression<T>,
{
    fn validate(&self, dialect: SqlDialect) -> Result<(), QueryError> {
        self.expression.validate(dialect)
    }

    fn render(&self, ctx: &mut ExprRenderCtx<'_>) -> Result<(), QueryError> {
        self.expression.render(ctx)
    }
}

impl IntoFunctionArgs for () {
    fn into_function_args(self) -> FunctionArgs {
        FunctionArgs::default()
    }
}

impl<T> IntoAnyExpr for Expr<T> {
    fn into_any_expr(self) -> FunctionArgs {
        FunctionArgs {
            nodes: vec![self.node],
        }
    }
}

impl<T> IntoAnyExpr for &Expr<T> {
    fn into_any_expr(self) -> FunctionArgs {
        FunctionArgs {
            nodes: vec![self.node.clone()],
        }
    }
}

impl<T> IntoAnyExpr for Column<T> {
    fn into_any_expr(self) -> FunctionArgs {
        self.into_expr().into_any_expr()
    }
}

impl<T> IntoAnyExpr for &Column<T> {
    fn into_any_expr(self) -> FunctionArgs {
        self.into_expr().into_any_expr()
    }
}

impl<T> IntoAnyExpr for ProjectedColumn<T> {
    fn into_any_expr(self) -> FunctionArgs {
        self.into_expr().into_any_expr()
    }
}

impl<T> IntoAnyExpr for &ProjectedColumn<T> {
    fn into_any_expr(self) -> FunctionArgs {
        self.into_expr().into_any_expr()
    }
}

impl<T> IntoAnyExpr for Var<T> {
    fn into_any_expr(self) -> FunctionArgs {
        self.into_expr().into_any_expr()
    }
}

impl<T> IntoAnyExpr for &Var<T> {
    fn into_any_expr(self) -> FunctionArgs {
        self.into_expr().into_any_expr()
    }
}

macro_rules! impl_function_args {
    ($($name:ident : $index:tt),+ $(,)?) => {
        impl<$($name),+> IntoFunctionArgs for ($($name,)+)
        where
            $($name: IntoAnyExpr,)+
        {
            fn into_function_args(self) -> FunctionArgs {
                let mut nodes = Vec::new();
                $(nodes.extend(self.$index.into_any_expr().nodes);)+
                FunctionArgs { nodes }
            }
        }
    };
}

impl_function_args!(A: 0);
impl_function_args!(A: 0, B: 1);
impl_function_args!(A: 0, B: 1, C: 2);
impl_function_args!(A: 0, B: 1, C: 2, D: 3);
impl_function_args!(A: 0, B: 1, C: 2, D: 3, E: 4);
impl_function_args!(A: 0, B: 1, C: 2, D: 3, E: 4, F: 5);

use super::expr::IntoExpr;
