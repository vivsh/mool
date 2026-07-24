//! Portable JSON expression helpers.

use crate::ArgValue;
use crate::placeholders::Dialect;
use crate::types::Json;

use super::super::expr::{Expr, ExprNode, IntoExpr, Predicate, ValueNode};
use super::super::extension::{DbExpression, ExprRenderCtx, FunctionArgs, custom};
use crate::QueryError;

/// Creates a typed JSON literal expression.
pub fn value(value: serde_json::Value) -> Expr<Json> {
    Expr::new(ExprNode::Value(ValueNode::Val {
        name: None,
        rust_type: std::any::type_name::<serde_json::Value>(),
        value: ArgValue::new(value),
    }))
}

/// Extracts a JSON value at `path`.
pub fn get(json: impl IntoExpr<Json>, path: impl Into<String>) -> Expr<Json> {
    custom(JsonPathExpr::new(JsonPathOp::Get, json.into_expr(), path))
}

/// Extracts a text value at `path`.
pub fn text(json: impl IntoExpr<Json>, path: impl Into<String>) -> Expr<String> {
    custom(JsonPathExpr::new(JsonPathOp::Text, json.into_expr(), path))
}

/// Checks whether a JSON path exists.
pub fn exists(json: impl IntoExpr<Json>, path: impl Into<String>) -> Predicate {
    custom::<bool, _>(JsonPathExpr::new(
        JsonPathOp::Exists,
        json.into_expr(),
        path,
    ))
    .into_predicate()
}

/// Returns the JSON type name at `path`.
pub fn json_type(json: impl IntoExpr<Json>, path: impl Into<String>) -> Expr<String> {
    custom(JsonPathExpr::new(JsonPathOp::Type, json.into_expr(), path))
}

/// Returns the JSON array length at `path`.
pub fn array_length(json: impl IntoExpr<Json>, path: impl Into<String>) -> Expr<i64> {
    custom(JsonPathExpr::new(
        JsonPathOp::ArrayLength,
        json.into_expr(),
        path,
    ))
}

#[derive(Clone)]
struct JsonPathExpr {
    op: JsonPathOp,
    path: String,
    args: FunctionArgs,
}

#[derive(Clone, Copy)]
enum JsonPathOp {
    Get,
    Text,
    Exists,
    Type,
    ArrayLength,
}

impl JsonPathExpr {
    fn new(op: JsonPathOp, json: Expr<Json>, path: impl Into<String>) -> Self {
        Self {
            op,
            path: path.into(),
            args: FunctionArgs::new((json,)),
        }
    }

    fn pg_path(&self) -> String {
        let parts = path_parts(&self.path);
        format!("'{{{}}}'", parts.join(","))
    }

    fn json_path(&self) -> String {
        if self.path.starts_with('$') {
            return quote_sql(&self.path);
        }
        let path = path_parts(&self.path)
            .into_iter()
            .fold("$".to_string(), |mut out, part| {
                out.push('.');
                out.push_str(&part);
                out
            });
        quote_sql(&path)
    }
}

impl DbExpression<Json> for JsonPathExpr {
    fn args(&self) -> FunctionArgs {
        self.args.clone()
    }

    fn render(&self, ctx: &mut ExprRenderCtx<'_>) -> Result<(), QueryError> {
        let (prefix, suffix) = match (ctx.dialect(), self.op) {
            (Dialect::Postgres, JsonPathOp::Get) => ("(", format!(" #> {})", self.pg_path())),
            (Dialect::Sqlite, JsonPathOp::Get) => {
                ("json_extract(", format!(", {})", self.json_path()))
            }
            (Dialect::Mysql, JsonPathOp::Get) => {
                ("JSON_EXTRACT(", format!(", {})", self.json_path()))
            }
            _ => return Err(unsupported(ctx.dialect(), "JSON value extraction")),
        };
        push_wrapped_arg(ctx, prefix, 0, &suffix)
    }
}

impl DbExpression<String> for JsonPathExpr {
    fn args(&self) -> FunctionArgs {
        self.args.clone()
    }

    fn render(&self, ctx: &mut ExprRenderCtx<'_>) -> Result<(), QueryError> {
        let (prefix, suffix) = match (ctx.dialect(), self.op) {
            (Dialect::Postgres, JsonPathOp::Text) => ("(", format!(" #>> {})", self.pg_path())),
            (Dialect::Sqlite, JsonPathOp::Text) => {
                ("json_extract(", format!(", {})", self.json_path()))
            }
            (Dialect::Mysql, JsonPathOp::Text) => (
                "JSON_UNQUOTE(JSON_EXTRACT(",
                format!(", {}))", self.json_path()),
            ),
            (Dialect::Postgres, JsonPathOp::Type) => {
                ("jsonb_typeof(", format!(" #> {}))", self.pg_path()))
            }
            (Dialect::Sqlite, JsonPathOp::Type) => {
                ("json_type(", format!(", {})", self.json_path()))
            }
            (Dialect::Mysql, JsonPathOp::Type) => (
                "JSON_TYPE(JSON_EXTRACT(",
                format!(", {}))", self.json_path()),
            ),
            _ => return Err(unsupported(ctx.dialect(), "JSON text expression")),
        };
        push_wrapped_arg(ctx, prefix, 0, &suffix)
    }
}

impl DbExpression<bool> for JsonPathExpr {
    fn args(&self) -> FunctionArgs {
        self.args.clone()
    }

    fn render(&self, ctx: &mut ExprRenderCtx<'_>) -> Result<(), QueryError> {
        let (prefix, suffix) = match (ctx.dialect(), self.op) {
            (Dialect::Postgres, JsonPathOp::Exists) => {
                ("(", format!(" #> {}) IS NOT NULL", self.pg_path()))
            }
            (Dialect::Sqlite, JsonPathOp::Exists) => {
                ("json_type(", format!(", {}) IS NOT NULL", self.json_path()))
            }
            (Dialect::Mysql, JsonPathOp::Exists) => (
                "JSON_CONTAINS_PATH(",
                format!(", 'one', {})", self.json_path()),
            ),
            _ => return Err(unsupported(ctx.dialect(), "JSON path existence")),
        };
        push_wrapped_arg(ctx, prefix, 0, &suffix)
    }
}

impl DbExpression<i64> for JsonPathExpr {
    fn args(&self) -> FunctionArgs {
        self.args.clone()
    }

    fn render(&self, ctx: &mut ExprRenderCtx<'_>) -> Result<(), QueryError> {
        let (prefix, suffix) = match (ctx.dialect(), self.op) {
            (Dialect::Postgres, JsonPathOp::ArrayLength) => {
                ("jsonb_array_length(", format!(" #> {}))", self.pg_path()))
            }
            (Dialect::Sqlite, JsonPathOp::ArrayLength) => {
                ("json_array_length(", format!(", {})", self.json_path()))
            }
            (Dialect::Mysql, JsonPathOp::ArrayLength) => (
                "JSON_LENGTH(JSON_EXTRACT(",
                format!(", {}))", self.json_path()),
            ),
            _ => return Err(unsupported(ctx.dialect(), "JSON array length")),
        };
        push_wrapped_arg(ctx, prefix, 0, &suffix)
    }
}

fn push_wrapped_arg(
    ctx: &mut ExprRenderCtx<'_>,
    prefix: &str,
    index: usize,
    suffix: &str,
) -> Result<(), QueryError> {
    ctx.push_sql(prefix);
    ctx.push_arg(index)?;
    ctx.push_sql(suffix);
    Ok(())
}

fn path_parts(path: &str) -> Vec<String> {
    path.trim_start_matches('$')
        .trim_start_matches('.')
        .split('.')
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect()
}

fn quote_sql(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn unsupported(dialect: Dialect, feature: &'static str) -> QueryError {
    QueryError::Unsupported {
        dialect: dialect.name(),
        feature,
    }
}

/// PostgreSQL-specific JSON helpers.
#[cfg(feature = "postgres")]
pub mod postgres {
    use crate::QueryError;
    use crate::queries::extension::{DbExpression, ExprRenderCtx, FunctionArgs, custom};
    use crate::queries::{IntoExpr, Predicate};
    use crate::types::Json;

    /// Creates a PostgreSQL `left @> right` JSONB containment predicate.
    pub fn contains(left: impl IntoExpr<Json>, right: impl IntoExpr<Json>) -> Predicate {
        custom::<bool, _>(JsonbContains {
            args: FunctionArgs::new((left.into_expr(), right.into_expr())),
        })
        .into_predicate()
    }

    #[derive(Clone)]
    struct JsonbContains {
        args: FunctionArgs,
    }

    impl DbExpression<bool> for JsonbContains {
        fn args(&self) -> FunctionArgs {
            self.args.clone()
        }

        fn render(&self, ctx: &mut ExprRenderCtx<'_>) -> Result<(), QueryError> {
            ctx.push_sql("(");
            ctx.push_arg(0)?;
            ctx.push_sql(" @> ");
            ctx.push_arg(1)?;
            ctx.push_sql(")");
            Ok(())
        }
    }
}
