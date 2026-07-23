//! PostgreSQL typed-query dialect renderer.

use super::super::expr::ColumnRef;
use super::{DialectFeature, DialectRenderer, common};
use crate::QueryError;

pub(super) struct PostgresSpec;

impl DialectRenderer for PostgresSpec {
    fn placeholder(&self, position: usize) -> String {
        format!("${position}")
    }

    fn validate_feature(&self, _feature: DialectFeature) -> Result<(), QueryError> {
        Ok(())
    }

    fn render_upsert(
        &self,
        conflict: &[ColumnRef],
        update_columns: &[&str],
    ) -> Result<String, QueryError> {
        common::render_on_conflict(conflict, update_columns)
    }

    fn render_ignore_conflicts(&self, conflict: &[ColumnRef]) -> Result<String, QueryError> {
        common::render_ignore_conflicts(conflict)
    }
}
