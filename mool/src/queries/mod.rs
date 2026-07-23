//! Source-first SQL AST for typed query construction.
//!
//! This module owns Vyuh's typed query builder. It keeps raw SQL and string
//! paths out of the typed query surface; advanced SQL remains the job of
//! `db::query(...)`.
//!
//! The public surface is intentionally small and re-exported from this module.
//! Implementation details live in private modules for planning, handles,
//! sources, expressions, dialect rendering, binding, and validation.

mod api;
mod batch;
mod executables;
mod expr;
mod extension;
mod functions;
mod handles;
mod output;
mod plan;
mod planning;
mod scope;
mod set;
mod source;
mod traits;
mod values;
mod window;

mod binds;
mod dialect;
mod render;
mod render_window;
mod terminals;
mod validate;

pub(super) const GENERATED_PREFIX: &str = "__typed_";

pub use api::{backref, from, many_to_many, meta, out, val, var};
pub(crate) use batch::InsertConflict;
pub use batch::{BatchPlan, BatchStatementPlan, ColumnSet};
pub use expr::{ColumnRef, Expr, IntoExpr};
pub(crate) use expr::{many_to_many_exists, relation_aggregate, relation_exists};
pub use extension::{DbExpression, DbFunction, ExprRenderCtx, FunctionArgs, IntoFunctionArgs};
pub use plan::{ParamSource, ParamSpec, QueryPlan};
pub use source::{SourceKind, SourceMeta};

/// SQL function and expression helpers for typed queries.
pub mod funcs {
    pub use super::extension::{custom, func};
    pub use super::functions::aggregate::{avg, count, count_all, max, min, sum};
    #[cfg(feature = "postgres")]
    pub use super::functions::arrays as array;
    pub use super::functions::cast::{CastTarget, cast};
    pub use super::functions::common::{
        case, coalesce, cume_dist, dense_rank, first_value, lag, lag_by, lag_or, last_value, lead,
        lead_by, lead_or, now, nth_value, ntile, percent_rank, rank, row_number,
    };
    pub use super::functions::json;
    #[cfg(feature = "postgres")]
    pub use super::functions::postgres;
    pub use super::window::{
        current_row, following, preceding, range_between, rows_between, unbounded_following,
        unbounded_preceding, window,
    };
}

#[doc(hidden)]
pub use api::__private;
#[doc(hidden)]
pub use executables::{
    All, BatchInsert, BatchUpdate, BatchUpsert, Count, Delete, Exists, First, Insert, One,
    OwnedBatchInsert, OwnedBatchUpdate, OwnedBatchUpsert, OwnedInsert, OwnedUpdate,
    ReturningBatchInsert, ReturningBatchUpdate, ReturningBatchUpsert, ReturningDelete,
    ReturningInsert, ReturningUpdate, Scalar, Slice, Update,
};
#[cfg(feature = "postgres")]
#[doc(hidden)]
pub use executables::{
    OwnedPgUnnestBatchInsert, OwnedPgUnnestBatchUpsert, PgUnnestBatchInsert, PgUnnestBatchUpsert,
    ReturningPgUnnestBatchInsert, ReturningPgUnnestBatchUpsert,
};
#[doc(hidden)]
pub use expr::{OrderExpr, Predicate};
#[doc(hidden)]
pub use handles::{Column, ModelTable, Var};
#[doc(hidden)]
pub use output::{HasOutputCols, IntoOutputTarget, OutputColumn, OutputSource};
#[doc(hidden)]
pub use scope::{QueryScope, ReturningScope};
#[doc(hidden)]
pub use set::{Set, SetOp};
#[doc(hidden)]
pub use source::{Cte, Picked, ProjectedColumn, ProjectionSource, Subquery};
#[doc(hidden)]
pub use traits::Projectable;
#[doc(hidden)]
pub use values::{NoRecord, WriteInput, WriteValues};
#[doc(hidden)]
pub use window::{FrameBound, WindowFrame, WindowSpec};
