//! Shared batch-write planning types and typed column collections.

use std::ops::Range;

use super::QueryPlan;
use super::expr::ColumnRef;
use super::traits::IntoColumnRef;
use crate::QueryError;

#[derive(Clone, Default)]
pub(crate) enum InsertConflict {
    #[default]
    None,
    #[cfg(any(feature = "postgres", feature = "sqlite"))]
    Ignore(Option<Vec<ColumnRef>>),
    #[cfg(any(feature = "mysql", feature = "mariadb"))]
    IgnoreErrors,
}

#[derive(Clone)]
pub(crate) enum BatchInsertMode {
    Insert,
    #[cfg(any(feature = "postgres", feature = "sqlite"))]
    Ignore(Option<Vec<ColumnRef>>),
    #[cfg(any(feature = "mysql", feature = "mariadb"))]
    IgnoreErrors,
    Upsert {
        conflict: Vec<ColumnRef>,
        update_columns: Option<Vec<ColumnRef>>,
    },
}

/// Ordered query plans produced by one logical batch operation.
#[derive(Clone, Debug)]
pub struct BatchPlan {
    statements: Vec<BatchStatementPlan>,
}

impl BatchPlan {
    pub(crate) fn new(statements: Vec<BatchStatementPlan>) -> Self {
        Self { statements }
    }

    /// Returns the statements in execution order.
    pub fn statements(&self) -> &[BatchStatementPlan] {
        &self.statements
    }

    /// Consumes the batch plan and returns its statement plans.
    pub fn into_statements(self) -> Vec<BatchStatementPlan> {
        self.statements
    }

    /// Returns whether this operation resolves to one SQL statement.
    pub fn is_single_statement(&self) -> bool {
        self.statements.len() == 1
    }
}

/// One SQL statement and the input rows it represents.
#[derive(Clone, Debug)]
pub struct BatchStatementPlan {
    plan: QueryPlan,
    rows: Range<usize>,
}

impl BatchStatementPlan {
    pub(crate) fn new(plan: QueryPlan, rows: Range<usize>) -> Self {
        Self { plan, rows }
    }

    /// Returns the rendered query plan.
    pub fn plan(&self) -> &QueryPlan {
        &self.plan
    }

    /// Returns the input row range represented by this statement.
    pub fn rows(&self) -> Range<usize> {
        self.rows.clone()
    }
}

mod sealed {
    use super::{ColumnRef, IntoColumnRef};
    use crate::queries::Column;

    pub trait Sealed {}

    impl<T> Sealed for Column<T> {}
    impl<T> Sealed for &Column<T> {}
    impl Sealed for ColumnRef {}

    impl<C, const N: usize> Sealed for [C; N] where C: IntoColumnRef + Sealed {}
    impl<C> Sealed for Vec<C> where C: IntoColumnRef + Sealed {}

    macro_rules! impl_sealed_tuple {
        ($($name:ident),+) => {
            impl<$($name),+> Sealed for ($($name,)+)
            where
                $($name: IntoColumnRef + Sealed),+
            {}
        };
    }

    impl_sealed_tuple!(A);
    impl_sealed_tuple!(A, B);
    impl_sealed_tuple!(A, B, C);
    impl_sealed_tuple!(A, B, C, D);
    impl_sealed_tuple!(A, B, C, D, E);
    impl_sealed_tuple!(A, B, C, D, E, F);
    impl_sealed_tuple!(A, B, C, D, E, F, G);
    impl_sealed_tuple!(A, B, C, D, E, F, G, H);
    impl_sealed_tuple!(A, B, C, D, E, F, G, H, I);
    impl_sealed_tuple!(A, B, C, D, E, F, G, H, I, J);
    impl_sealed_tuple!(A, B, C, D, E, F, G, H, I, J, K);
    impl_sealed_tuple!(A, B, C, D, E, F, G, H, I, J, K, L);
    impl_sealed_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M);
    impl_sealed_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M, N);
    impl_sealed_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O);
    impl_sealed_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P);
}

/// A sealed typed collection of columns used by conflict and update clauses.
pub trait ColumnSet: sealed::Sealed {
    #[doc(hidden)]
    fn into_column_refs(self) -> Vec<ColumnRef>;
}

impl<C> ColumnSet for C
where
    C: IntoColumnRef + sealed::Sealed,
{
    fn into_column_refs(self) -> Vec<ColumnRef> {
        vec![self.into_column_ref()]
    }
}

impl<C, const N: usize> ColumnSet for [C; N]
where
    C: IntoColumnRef + sealed::Sealed,
{
    fn into_column_refs(self) -> Vec<ColumnRef> {
        self.into_iter()
            .map(IntoColumnRef::into_column_ref)
            .collect()
    }
}

impl<C> ColumnSet for Vec<C>
where
    C: IntoColumnRef + sealed::Sealed,
{
    fn into_column_refs(self) -> Vec<ColumnRef> {
        self.into_iter()
            .map(IntoColumnRef::into_column_ref)
            .collect()
    }
}

macro_rules! impl_column_set_tuple {
    ($($name:ident:$index:tt),+) => {
        impl<$($name),+> ColumnSet for ($($name,)+)
        where
            $($name: IntoColumnRef + sealed::Sealed),+
        {
            fn into_column_refs(self) -> Vec<ColumnRef> {
                vec![$(self.$index.into_column_ref()),+]
            }
        }
    };
}

impl_column_set_tuple!(A:0);
impl_column_set_tuple!(A:0, B:1);
impl_column_set_tuple!(A:0, B:1, C:2);
impl_column_set_tuple!(A:0, B:1, C:2, D:3);
impl_column_set_tuple!(A:0, B:1, C:2, D:3, E:4);
impl_column_set_tuple!(A:0, B:1, C:2, D:3, E:4, F:5);
impl_column_set_tuple!(A:0, B:1, C:2, D:3, E:4, F:5, G:6);
impl_column_set_tuple!(A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7);
impl_column_set_tuple!(A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8);
impl_column_set_tuple!(A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, J:9);
impl_column_set_tuple!(A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, J:9, K:10);
impl_column_set_tuple!(A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, J:9, K:10, L:11);
impl_column_set_tuple!(A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, J:9, K:10, L:11, M:12);
impl_column_set_tuple!(A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, J:9, K:10, L:11, M:12, N:13);
impl_column_set_tuple!(A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, J:9, K:10, L:11, M:12, N:13, O:14);
impl_column_set_tuple!(A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, J:9, K:10, L:11, M:12, N:13, O:14, P:15);

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct BatchPolicy {
    size: Option<usize>,
    single_statement: bool,
}

impl BatchPolicy {
    pub(crate) fn with_size(mut self, size: usize) -> Self {
        self.size = Some(size);
        self
    }

    pub(crate) fn single_statement(mut self) -> Self {
        self.single_statement = true;
        self
    }

    /// Resolves deterministic input ranges without silently exceeding limits.
    pub(crate) fn ranges(
        self,
        operation: &'static str,
        rows: usize,
        binds_per_row: usize,
    ) -> Result<Vec<Range<usize>>, QueryError> {
        self.ranges_with_overhead(operation, rows, binds_per_row, 0)
    }

    /// Resolves ranges while reserving parameters used outside row payloads.
    pub(crate) fn ranges_with_overhead(
        self,
        operation: &'static str,
        rows: usize,
        binds_per_row: usize,
        fixed_parameters: usize,
    ) -> Result<Vec<Range<usize>>, QueryError> {
        self.validate_common(operation, rows)?;
        if binds_per_row == 0 {
            return Err(QueryError::BindError("no bindable columns".to_string()));
        }
        let safe = safe_rows(operation, binds_per_row, fixed_parameters)?;
        if self.single_statement && rows > safe {
            return Err(batch_too_large_error(
                operation,
                rows,
                binds_per_row,
                fixed_parameters,
            )?);
        }
        let size = self.size.map_or(safe, |requested| requested.min(safe));
        Ok((0..rows)
            .step_by(size)
            .map(|start| start..(start + size).min(rows))
            .collect())
    }

    #[cfg(feature = "postgres")]
    pub(crate) fn unnest_ranges(
        self,
        operation: &'static str,
        rows: usize,
    ) -> Result<Vec<Range<usize>>, QueryError> {
        self.validate_common(operation, rows)?;
        let size = self.size.unwrap_or(rows);
        Ok((0..rows)
            .step_by(size)
            .map(|start| start..(start + size).min(rows))
            .collect())
    }

    /// Rejects empty inputs and internally conflicting batch policies.
    fn validate_common(self, operation: &'static str, rows: usize) -> Result<(), QueryError> {
        if rows == 0 {
            return Err(QueryError::EmptyBatch { operation });
        }
        if self.single_statement && self.size.is_some() {
            return Err(QueryError::InvalidModifier {
                modifier: "batch_size",
                terminal: "single_statement batch",
            });
        }
        if self.size == Some(0) {
            return Err(QueryError::InvalidBatchSize(0));
        }
        Ok(())
    }
}

fn checked_product(rows: usize, columns: usize) -> Result<usize, QueryError> {
    rows.checked_mul(columns)
        .ok_or(QueryError::BatchParameterOverflow { rows, columns })
}

/// Computes the largest row count after reserving fixed statement parameters.
fn safe_rows(
    operation: &'static str,
    columns: usize,
    fixed_parameters: usize,
) -> Result<usize, QueryError> {
    let available = crate::backend::PARAMETER_LIMIT
        .checked_sub(fixed_parameters)
        .unwrap_or_default();
    let safe = available / columns;
    if safe == 0 {
        return Err(batch_too_large_error(
            operation,
            1,
            columns,
            fixed_parameters,
        )?);
    }
    Ok(safe)
}

/// Builds an oversized-statement error while detecting arithmetic overflow.
fn batch_too_large_error(
    operation: &'static str,
    rows: usize,
    columns: usize,
    fixed_parameters: usize,
) -> Result<QueryError, QueryError> {
    let required_parameters = checked_product(rows, columns)?
        .checked_add(fixed_parameters)
        .ok_or(QueryError::BatchParameterOverflow { rows, columns })?;
    Ok(QueryError::BatchTooLarge {
        operation,
        rows,
        columns,
        required_parameters,
        parameter_limit: crate::backend::PARAMETER_LIMIT,
    })
}
