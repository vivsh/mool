//! MariaDB typed-query dialect renderer.

use crate::placeholders::Dialect;

use super::super::expr::ColumnRef;
use super::super::validate::validate_identifier;
use super::{DialectFeature, DialectRenderer, common};
use crate::QueryError;

pub(super) struct MariadbSpec;

impl DialectRenderer for MariadbSpec {
    fn placeholder(&self, _position: usize) -> String {
        "?".to_string()
    }

    fn validate_feature(&self, feature: DialectFeature) -> Result<(), QueryError> {
        match feature {
            DialectFeature::Returning | DialectFeature::Ilike => {
                Err(common::unsupported(Dialect::Mariadb, feature.name()))
            }
            DialectFeature::Upsert | DialectFeature::WindowFunctions => Ok(()),
        }
    }

    fn render_upsert(
        &self,
        _conflict: &[ColumnRef],
        update_columns: &[&str],
    ) -> Result<String, QueryError> {
        if update_columns.is_empty() {
            return Err(QueryError::BindError(
                "mariadb upsert requires at least one non-conflict bind column".to_string(),
            ));
        }
        let assignments = update_columns
            .iter()
            .map(|column| {
                validate_identifier(column)?;
                Ok(format!("{column} = VALUES({column})"))
            })
            .collect::<Result<Vec<_>, QueryError>>()?;
        Ok(format!(
            " ON DUPLICATE KEY UPDATE {}",
            assignments.join(", ")
        ))
    }
}
