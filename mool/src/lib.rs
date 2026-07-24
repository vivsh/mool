//! Mool's public API.

#[cfg(mool_has_backend)]
mod argvalue;
pub mod backend;
#[cfg(not(mool_has_backend))]
mod backendless;
#[cfg(mool_has_backend)]
mod commons;
#[cfg(mool_has_backend)]
pub mod datetime;
pub mod enums;
#[cfg(mool_has_backend)]
mod executor;
#[cfg(mool_has_backend)]
pub mod filters;
#[cfg(mool_has_backend)]
mod interfaces;
pub mod migrations;
#[cfg(mool_has_backend)]
mod page;
#[cfg(mool_has_backend)]
mod placeholders;
pub mod prelude;
#[cfg(mool_has_backend)]
pub mod queries;
#[cfg(mool_has_backend)]
mod query_error;
#[cfg(mool_has_backend)]
mod raw;
#[cfg(mool_has_backend)]
pub mod relations;
pub mod schema;
#[cfg(mool_has_backend)]
mod statement;
#[cfg(mool_has_backend)]
pub mod types;

pub use gaman;
pub use sqlx;

extern crate self as mool;

#[cfg(all(mool_has_backend, any(test, debug_assertions, feature = "mock")))]
pub mod mock;

#[cfg(mool_has_backend)]
pub use argvalue::ArgValue;
#[cfg(not(mool_has_backend))]
pub use backendless::{DbConf, DbError, DbOperation, DbPool, IntegrityKind, QueryError};
pub use enums::{SqlEnum, SqlEnumError, SqlEnumStorage};
#[cfg(mool_has_backend)]
pub use enums::{SqlSchemaBuilder, schema};
#[cfg(mool_has_backend)]
pub use executor::*;
#[cfg(mool_has_backend)]
pub use filters::{FilterBuilder, Filterable};
#[cfg(mool_has_backend)]
pub use interfaces::{BatchRecord, Model, ModelSchema, Record, RecordSchema};
pub use mool_macros::{Filterable, Model, Record, SqlEnum};
#[cfg(mool_has_backend)]
pub use page::Page;
#[cfg(mool_has_backend)]
pub use placeholders::SqlDialect;
#[cfg(mool_has_backend)]
pub use queries::{
    DbExpression, DbFunction, Expr, ExprRenderCtx, FunctionArgs, ParamSource, ParamSpec, QueryPlan,
    SourceKind, SourceMeta, backref, from, funcs, many_to_many, meta, out, val, var,
};
#[cfg(mool_has_backend)]
pub use query_error::{LockMode, QueryError};
#[cfg(mool_has_backend)]
pub use raw::RawQuery;
#[cfg(mool_has_backend)]
pub use relations::{
    Backref, JoinColumn, JoinCtx, JoinRelation, JoinType, ManyBackref, ManyToMany, OneBackref,
    Prefetch, PrefetchKey, ReceivesPrefetch, ReferenceMeta, RelationCardinality, prefetch,
};
#[cfg(mool_has_backend)]
pub use statement::Statement;

/// Start a raw SQL query with named-bind support.
#[cfg(mool_has_backend)]
pub fn query(sql: &str) -> RawQuery {
    RawQuery::new(sql)
}
