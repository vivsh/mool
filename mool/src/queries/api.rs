//! Public typed-query constructors and hidden macro support helpers.
use std::any::type_name;
use std::marker::PhantomData;
use std::sync::Arc;

use crate::argvalue::ArgValue;
use crate::relations::{Backref, BackrefRef, ManyToMany, ManyToManyRef};

use super::expr::{Expr, ExprNode, ValueNode};
use super::handles::{ModelTable, Var, VarData, VarId};
use super::output::{HasOutputCols, OutputSource};
use super::scope::QueryScope;
use super::source::SourceMeta;
use super::traits::{HasCols, IntoSourceMeta, IntoTableSource};

/// Creates a typed placeholder handle.
pub fn var<T>() -> Var<T> {
    Var {
        data: Arc::new(VarData {
            id: VarId::next(),
            name: None,
            rust_type: type_name::<T>(),
        }),
        _marker: PhantomData,
    }
}

/// Creates an immediately bound typed SQL value.
pub fn val<T>(value: T) -> Expr<T>
where
    T: Clone
        + for<'q> sqlx::Encode<'q, crate::commons::Database>
        + sqlx::Type<crate::commons::Database>
        + Send
        + Sync
        + 'static,
{
    Expr::new(ExprNode::Value(ValueNode::Val {
        name: None,
        rust_type: type_name::<T>(),
        value: ArgValue::new(value),
    }))
}

/// Starts a source-first typed query scope.
pub fn from<S>(source: S) -> QueryScope
where
    S: IntoTableSource,
{
    QueryScope::new(source.into_table_source())
}

/// Returns read-only metadata for a typed query source.
pub fn meta<S>(source: S) -> SourceMeta
where
    S: IntoSourceMeta,
{
    source.source_meta()
}

/// Returns typed output targets for a record projection.
pub fn out<R>() -> <R as HasOutputCols>::OutputColumns
where
    R: HasOutputCols,
{
    R::output_columns(OutputSource::new::<R>())
}

/// Returns a typed reverse-relation helper for predicates and aggregates.
pub fn backref<R>(source: &ModelTable<R::From>) -> BackrefRef<R>
where
    R: Backref,
    R::From: HasCols,
{
    BackrefRef::new(source)
}

/// Returns a typed many-to-many relation helper for predicates.
pub fn many_to_many<R>(source: &ModelTable<R::From>) -> ManyToManyRef<R>
where
    R: ManyToMany,
    R::From: HasCols,
{
    ManyToManyRef::new(source)
}

/// Hidden construction helpers used by generated typed-query metadata.
#[doc(hidden)]
pub mod __private {
    use std::sync::Arc;

    pub use super::super::expr::{ColumnRef, IntoSourceColumn, in_list, not_in_list};
    pub use super::super::handles::{Column, ModelTable, Reference, Table, Var};
    pub use super::super::output::{HasOutputCols, OutputColumn, OutputSource};
    pub use super::super::source::{ProjectedColumn, ProjectionSource};
    pub use super::super::traits::HasCols;
    pub use super::super::traits::{IntoColumnRef, IntoTableSource, Projectable};

    /// Creates a table handle from macro-generated metadata.
    pub fn table(name: &str) -> Table {
        Table::new(None, name)
    }

    /// Creates a schema-qualified table handle from macro-generated metadata.
    pub fn table_schema(schema: &str, name: &str) -> Table {
        Table::new(Some(schema), name)
    }

    /// Creates a reference handle from macro-generated metadata.
    pub fn reference(name: &str) -> Reference {
        Reference {
            name: Arc::from(name),
        }
    }
}
