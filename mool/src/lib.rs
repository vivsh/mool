mod argvalue;
mod commons;
pub mod enums;
mod executor;
pub mod filters;
mod interfaces;
mod migrations;
mod page;
mod placeholders;
pub mod queries;
mod query_error;
mod raw;
pub mod relations;
mod statement;
pub mod types;

extern crate self as mool;

#[cfg(any(test, debug_assertions, feature = "mock"))]
pub mod mock;

pub use argvalue::ArgValue;
pub use commons::{Arguments, Database, Pool, QueryResult, Row};
pub use enums::{SqlEnum, SqlEnumError, SqlEnumStorage, SqlSchemaBuilder, schema};
pub use executor::*;
pub use filters::{FilterBuilder, Filterable};
pub use interfaces::{Model, ModelSchema, Record, RecordSchema};
pub use migrations::{
    Column, ColumnDesc, ColumnRef, ColumnType, Constraint, Dialect, FunctionDef, Index, IntoTable,
    Schema, SchemaBuilder, SchemaLoadError, Table, TableBuilder,
};
#[cfg(feature = "migrations")]
pub use migrations::{
    EmbeddedMigrations, MigrationError, MigrationRegistry, MigrationSource, SchemaSource,
    crate_migration, crate_schema, embedded_migrations, root_migration, root_schema,
};
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
pub use sqlx::test as test_db;
pub use statement::Statement;

/// Start a raw SQL query with named-bind support.
pub fn query(sql: &str) -> RawQuery {
    RawQuery::new(sql)
}
