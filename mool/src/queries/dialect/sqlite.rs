//! SQLite typed-query dialect renderer.

use crate::placeholders::Dialect;

use super::super::expr::ColumnRef;
use super::{DialectFeature, DialectRenderer, common};
use crate::QueryError;

pub(super) struct SqliteSpec;

impl DialectRenderer for SqliteSpec {
    fn placeholder(&self, _position: usize) -> String {
        "?".to_string()
    }

    fn validate_feature(&self, feature: DialectFeature) -> Result<(), QueryError> {
        match feature {
            DialectFeature::Ilike => Err(common::unsupported(Dialect::Sqlite, feature.name())),
            DialectFeature::Returning
            | DialectFeature::Upsert
            | DialectFeature::WindowFunctions => Ok(()),
        }
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
