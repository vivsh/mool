//! SQLite-only typed query capabilities.

use crate::Record;
use crate::queries::{QueryScope, ReturningScope};

/// SQLite write projections using `RETURNING`.
pub trait ReturningExt {
    /// Selects the typed record returned by a write terminal.
    fn returning<R>(self) -> ReturningScope<R>
    where
        R: Record;
}

impl ReturningExt for QueryScope {
    fn returning<R>(self) -> ReturningScope<R>
    where
        R: Record,
    {
        self.into_returning()
    }
}

/// Name of the selected backend.
pub const NAME: &str = "sqlite";

/// Conservative modern SQLite bind-parameter limit used for batch chunking.
pub const PARAMETER_LIMIT: usize = 32_766;
