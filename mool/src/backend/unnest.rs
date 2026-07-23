//! PostgreSQL columnar batch binding and `UNNEST` selection.

use crate::QueryError;
use crate::commons::Arguments;
use crate::queries::{
    BatchInsert, BatchUpsert, OwnedBatchInsert, OwnedBatchUpsert, OwnedPgUnnestBatchInsert,
    OwnedPgUnnestBatchUpsert, PgUnnestBatchInsert, PgUnnestBatchUpsert, ReturningBatchInsert,
    ReturningBatchUpsert, ReturningPgUnnestBatchInsert, ReturningPgUnnestBatchUpsert,
};

/// Column vectors that can be bound as PostgreSQL arrays.
#[doc(hidden)]
pub trait PgBatchColumns: Sized {
    /// Returns the common number of rows in the column vectors.
    fn row_count(&self) -> Result<usize, QueryError>;

    /// Returns the flattened number of bound columns.
    fn column_count(&self) -> usize;

    /// Binds each flattened column as one PostgreSQL array.
    fn bind(self, args: &mut Arguments<'static>) -> Result<(), QueryError>;
}

impl<T> PgBatchColumns for Vec<T>
where
    T: sqlx::postgres::PgHasArrayType
        + for<'q> sqlx::Encode<'q, sqlx::Postgres>
        + sqlx::Type<sqlx::Postgres>
        + Send
        + Sync
        + 'static,
{
    fn row_count(&self) -> Result<usize, QueryError> {
        Ok(self.len())
    }

    fn column_count(&self) -> usize {
        1
    }

    fn bind(self, args: &mut Arguments<'static>) -> Result<(), QueryError> {
        use sqlx::Arguments as _;
        args.add(self)
            .map_err(|error| QueryError::BindError(error.to_string()))
    }
}

macro_rules! impl_pg_batch_columns_tuple {
    ($first:ident:$first_index:tt $(, $name:ident:$index:tt)*) => {
        impl<$first, $($name),*> PgBatchColumns for ($first, $($name,)*)
        where
            $first: PgBatchColumns,
            $($name: PgBatchColumns),*
        {
            fn row_count(&self) -> Result<usize, QueryError> {
                let expected = self.$first_index.row_count()?;
                $(
                    let got = self.$index.row_count()?;
                    if got != expected {
                        return Err(QueryError::MismatchedBatchColumns { expected, got });
                    }
                )*
                Ok(expected)
            }

            fn column_count(&self) -> usize {
                self.$first_index.column_count() $(+ self.$index.column_count())*
            }

            fn bind(self, args: &mut Arguments<'static>) -> Result<(), QueryError> {
                self.$first_index.bind(args)?;
                $(self.$index.bind(args)?;)*
                Ok(())
            }
        }
    };
}

impl_pg_batch_columns_tuple!(A:0);
impl_pg_batch_columns_tuple!(A:0, B:1);
impl_pg_batch_columns_tuple!(A:0, B:1, C:2);
impl_pg_batch_columns_tuple!(A:0, B:1, C:2, D:3);
impl_pg_batch_columns_tuple!(A:0, B:1, C:2, D:3, E:4);
impl_pg_batch_columns_tuple!(A:0, B:1, C:2, D:3, E:4, F:5);
impl_pg_batch_columns_tuple!(A:0, B:1, C:2, D:3, E:4, F:5, G:6);
impl_pg_batch_columns_tuple!(A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7);
impl_pg_batch_columns_tuple!(A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8);
impl_pg_batch_columns_tuple!(A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, J:9);
impl_pg_batch_columns_tuple!(A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, J:9, K:10);
impl_pg_batch_columns_tuple!(A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, J:9, K:10, L:11);
impl_pg_batch_columns_tuple!(A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, J:9, K:10, L:11, M:12);
impl_pg_batch_columns_tuple!(A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, J:9, K:10, L:11, M:12, N:13);
impl_pg_batch_columns_tuple!(A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, J:9, K:10, L:11, M:12, N:13, O:14);
impl_pg_batch_columns_tuple!(A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, J:9, K:10, L:11, M:12, N:13, O:14, P:15);

/// Converts a normal batch insert or upsert into PostgreSQL `UNNEST` input.
pub trait PostgresUnnestExt {
    /// Resulting PostgreSQL-specific executable.
    type Output;

    /// Uses one PostgreSQL array parameter per writable record column.
    fn using_unnest(self) -> Self::Output;
}

impl<'a, T> PostgresUnnestExt for BatchInsert<'a, T> {
    type Output = PgUnnestBatchInsert<'a, T>;

    fn using_unnest(self) -> Self::Output {
        PgUnnestBatchInsert { inner: self }
    }
}

impl<'a, T> PostgresUnnestExt for BatchUpsert<'a, T> {
    type Output = PgUnnestBatchUpsert<'a, T>;

    fn using_unnest(self) -> Self::Output {
        PgUnnestBatchUpsert { inner: self }
    }
}

impl<T> PostgresUnnestExt for OwnedBatchInsert<T> {
    type Output = OwnedPgUnnestBatchInsert<T>;

    fn using_unnest(self) -> Self::Output {
        OwnedPgUnnestBatchInsert { inner: self }
    }
}

impl<T> PostgresUnnestExt for OwnedBatchUpsert<T> {
    type Output = OwnedPgUnnestBatchUpsert<T>;

    fn using_unnest(self) -> Self::Output {
        OwnedPgUnnestBatchUpsert { inner: self }
    }
}

impl<'a, R, T> PostgresUnnestExt for ReturningBatchInsert<'a, R, T> {
    type Output = ReturningPgUnnestBatchInsert<'a, R, T>;

    fn using_unnest(self) -> Self::Output {
        ReturningPgUnnestBatchInsert { inner: self }
    }
}

impl<'a, R, T> PostgresUnnestExt for ReturningBatchUpsert<'a, R, T> {
    type Output = ReturningPgUnnestBatchUpsert<'a, R, T>;

    fn using_unnest(self) -> Self::Output {
        ReturningPgUnnestBatchUpsert { inner: self }
    }
}
