mod argvalue;
pub mod backend;
mod commons;
pub mod enums;
mod executor;
pub mod filters;
mod interfaces;
pub mod migrations;
mod page;
mod placeholders;
pub mod prelude;
pub mod queries;
mod query_error;
mod raw;
pub mod relations;
pub mod schema;
mod statement;
pub mod types;

pub use gaman;
pub use sqlx;

extern crate self as mool;

#[cfg(any(test, debug_assertions, feature = "mock"))]
pub mod mock;

pub use argvalue::ArgValue;
pub use enums::{SqlEnum, SqlEnumError, SqlEnumStorage, SqlSchemaBuilder, schema};
pub use executor::*;
pub use filters::{FilterBuilder, Filterable};
pub use interfaces::{BatchRecord, Model, ModelSchema, Record, RecordSchema};
pub use mool_macros::{Filterable, Model, Record, SqlEnum};
pub use page::Page;
pub use queries::{
    DbExpression, DbFunction, Expr, ExprRenderCtx, FunctionArgs, ParamSource, ParamSpec, QueryPlan,
    SourceKind, SourceMeta, backref, from, funcs, many_to_many, meta, out, val, var,
};
pub use query_error::{LockMode, QueryError};
pub use raw::RawQuery;
pub use relations::{
    Backref, JoinColumn, JoinCtx, JoinRelation, JoinType, ManyBackref, ManyToMany, OneBackref,
    Prefetch, PrefetchKey, ReceivesPrefetch, ReferenceMeta, RelationCardinality, prefetch,
};
pub use statement::Statement;

/// Start a raw SQL query with named-bind support.
pub fn query(sql: &str) -> RawQuery {
    RawQuery::new(sql)
}
