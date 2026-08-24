//! Canonical imports for application code using Mool.

pub use crate::{gaman, migrations, schema, sqlx};

#[cfg(all(mool_has_backend, feature = "fuzzy-search"))]
pub use crate::FuzzyTextExt;
#[cfg(all(mool_has_backend, feature = "vector"))]
pub use crate::VectorSimilarityExt;
#[cfg(mool_has_backend)]
pub use crate::backend;
#[cfg(mool_has_backend)]
pub use crate::queries::{Expr, IntoExpr, ParamSource, ParamSpec, QueryPlan, funcs};
#[cfg(mool_has_backend)]
pub use crate::{
    Backref, DbConf, DbError, DbPool, DbSession, FilterBuilder, Filterable, JoinColumn,
    JoinRelation, JoinType, ManyBackref, ManyToMany, Model, ModelSchema, NoPrefix, OneBackref,
    Page, Pageable, Pagination, PaginationBuilder, PrefetchKey, QueryError, ReceivesPrefetch,
    Record, RecordSchema, Sort, SortBuilder, SortDirection, SortKey, SortParseError, Sortable,
    SqlEnum, Statement, TablePrefix, backref, from, many_to_many, meta, out, prefetch, query,
    random_order, val, var,
};
#[cfg(not(mool_has_backend))]
pub use crate::{DbConf, DbError, DbPool};

#[cfg(feature = "mysql")]
pub use crate::backend::IgnoreErrorsExt;
#[cfg(feature = "mariadb")]
pub use crate::backend::IgnoreErrorsExt;
#[cfg(all(feature = "postgres", feature = "fuzzy-search"))]
pub use crate::backend::PostgresTrigramExt;
#[cfg(all(feature = "postgres", feature = "vector"))]
pub use crate::backend::PostgresVectorExt;
#[cfg(feature = "mariadb")]
pub use crate::backend::RowLockExt;
#[cfg(feature = "postgres")]
pub use crate::backend::{DistinctOnExt, LockWaitExt, ReturningExt, RowLockExt, TextSearchExt};
#[cfg(feature = "postgres")]
pub use crate::backend::{IgnoreConflictsExt, PostgresUnnestExt};
#[cfg(feature = "sqlite")]
pub use crate::backend::{IgnoreConflictsExt, ReturningExt};
#[cfg(feature = "mysql")]
pub use crate::backend::{LockWaitExt, RowLockExt};
#[cfg(all(feature = "text-search", any(feature = "mysql", feature = "mariadb")))]
pub use crate::backend::{MySqlFullTextExt, MySqlFullTextMode};
#[cfg(all(feature = "postgres", feature = "text-search"))]
pub use crate::backend::{PostgresFullTextExt, PostgresTextQueryMode};
