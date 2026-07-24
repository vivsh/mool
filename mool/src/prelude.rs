//! Canonical imports for application code using Mool.

pub use crate::{gaman, migrations, schema, sqlx};

#[cfg(mool_has_backend)]
pub use crate::backend;
#[cfg(mool_has_backend)]
pub use crate::queries::{Expr, IntoExpr, ParamSource, ParamSpec, QueryPlan, funcs};
#[cfg(mool_has_backend)]
pub use crate::{
    Backref, DbConf, DbError, DbPool, DbSession, FilterBuilder, Filterable, JoinColumn,
    JoinRelation, JoinType, ManyBackref, ManyToMany, Model, ModelSchema, OneBackref, Page,
    PrefetchKey, QueryError, ReceivesPrefetch, Record, RecordSchema, SqlEnum, Statement, backref,
    from, many_to_many, meta, out, prefetch, query, val, var,
};

#[cfg(feature = "mysql")]
pub use crate::backend::IgnoreErrorsExt;
#[cfg(feature = "mariadb")]
pub use crate::backend::IgnoreErrorsExt;
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
