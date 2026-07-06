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

    fn render(&self, ctx: &mut ExprRenderCtx<'_>) -> Result<String, QueryError> {
        let json = ctx.arg(0)?;
        match (ctx.dialect(), self.op) {
            (Dialect::Postgres, JsonPathOp::Get) => Ok(format!("({json} #> {})", self.pg_path())),
            (Dialect::Sqlite, JsonPathOp::Get) => {
                Ok(format!("json_extract({json}, {})", self.json_path()))
            }
            (Dialect::Mysql, JsonPathOp::Get) => {
                Ok(format!("JSON_EXTRACT({json}, {})", self.json_path()))
            }
            _ => Err(unsupported(ctx.dialect(), "JSON value extraction")),
        }
    }
}

impl DbExpression<String> for JsonPathExpr {
    fn args(&self) -> FunctionArgs {
        self.args.clone()
    }

    fn render(&self, ctx: &mut ExprRenderCtx<'_>) -> Result<String, QueryError> {
        let json = ctx.arg(0)?;
        match (ctx.dialect(), self.op) {
            (Dialect::Postgres, JsonPathOp::Text) => Ok(format!("({json} #>> {})", self.pg_path())),
            (Dialect::Sqlite, JsonPathOp::Text) => {
                Ok(format!("json_extract({json}, {})", self.json_path()))
            }
            (Dialect::Mysql, JsonPathOp::Text) => Ok(format!(
                "JSON_UNQUOTE(JSON_EXTRACT({json}, {}))",
                self.json_path()
            )),
            (Dialect::Postgres, JsonPathOp::Type) => {
                Ok(format!("jsonb_typeof({json} #> {})", self.pg_path()))
            }
            (Dialect::Sqlite, JsonPathOp::Type) => {
                Ok(format!("json_type({json}, {})", self.json_path()))
            }
            (Dialect::Mysql, JsonPathOp::Type) => Ok(format!(
                "JSON_TYPE(JSON_EXTRACT({json}, {}))",
                self.json_path()
            )),
            _ => Err(unsupported(ctx.dialect(), "JSON text expression")),
        }
    }
}

impl DbExpression<bool> for JsonPathExpr {
    fn args(&self) -> FunctionArgs {
        self.args.clone()
    }

    fn render(&self, ctx: &mut ExprRenderCtx<'_>) -> Result<String, QueryError> {
        let json = ctx.arg(0)?;
        match (ctx.dialect(), self.op) {
            (Dialect::Postgres, JsonPathOp::Exists) => {
                Ok(format!("({json} #> {}) IS NOT NULL", self.pg_path()))
            }
            (Dialect::Sqlite, JsonPathOp::Exists) => Ok(format!(
                "json_type({json}, {}) IS NOT NULL",
                self.json_path()
            )),
            (Dialect::Mysql, JsonPathOp::Exists) => Ok(format!(
                "JSON_CONTAINS_PATH({json}, 'one', {})",
                self.json_path()
            )),
            _ => Err(unsupported(ctx.dialect(), "JSON path existence")),
        }
    }
}

impl DbExpression<i64> for JsonPathExpr {
    fn args(&self) -> FunctionArgs {
        self.args.clone()
    }

    fn render(&self, ctx: &mut ExprRenderCtx<'_>) -> Result<String, QueryError> {
        let json = ctx.arg(0)?;
        match (ctx.dialect(), self.op) {
            (Dialect::Postgres, JsonPathOp::ArrayLength) => {
                Ok(format!("jsonb_array_length({json} #> {})", self.pg_path()))
            }
            (Dialect::Sqlite, JsonPathOp::ArrayLength) => {
                Ok(format!("json_array_length({json}, {})", self.json_path()))
            }
            (Dialect::Mysql, JsonPathOp::ArrayLength) => Ok(format!(
                "JSON_LENGTH(JSON_EXTRACT({json}, {}))",
                self.json_path()
            )),
            _ => Err(unsupported(ctx.dialect(), "JSON array length")),
        }
    }
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

fn unsupported(dialect: Dialect, feature: &str) -> QueryError {
    QueryError::BindError(format!("{feature} is not supported for {dialect:?}"))
}

/// PostgreSQL-specific JSON helpers.
pub mod postgres {
    use crate::QueryError;
    use crate::placeholders::Dialect;
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

        fn validate(&self, dialect: Dialect) -> Result<(), QueryError> {
            if dialect == Dialect::Postgres {
                return Ok(());
            }
            Err(QueryError::BindError(format!(
                "jsonb containment is not supported for {}",
                dialect_name(dialect)
            )))
        }

        fn render(&self, ctx: &mut ExprRenderCtx<'_>) -> Result<String, QueryError> {
            Ok(format!("({} @> {})", ctx.arg(0)?, ctx.arg(1)?))
        }
    }

    fn dialect_name(dialect: Dialect) -> &'static str {
        match dialect {
            Dialect::Postgres => "postgres",
            Dialect::Sqlite => "sqlite",
            Dialect::Mysql => "mysql",
        }
    }
}
