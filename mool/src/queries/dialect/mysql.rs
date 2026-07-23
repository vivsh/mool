//! MySQL typed-query dialect renderer.

use crate::placeholders::Dialect;

use super::super::expr::ColumnRef;
use super::super::validate::validate_identifier;
use super::{DialectFeature, DialectRenderer, common};
use crate::QueryError;

pub(super) struct MysqlSpec;

impl DialectRenderer for MysqlSpec {
    fn placeholder(&self, _position: usize) -> String {
        "?".to_string()
    }

    fn validate_feature(&self, feature: DialectFeature) -> Result<(), QueryError> {
        match feature {
            DialectFeature::Returning | DialectFeature::Ilike => {
                Err(common::unsupported(Dialect::Mysql, feature.name()))
            }
            DialectFeature::Upsert | DialectFeature::WindowFunctions => Ok(()),
        }
    }

    fn render_upsert(
        &self,
        _conflict: &[ColumnRef],
        update_columns: &[&str],
    ) -> Result<String, QueryError> {
        validate_mysql_update(update_columns)?;
        Ok(format!(
            " ON DUPLICATE KEY UPDATE {}",
            render_mysql_update(update_columns)?
        ))
    }
}

fn validate_mysql_update(update_columns: &[&str]) -> Result<(), QueryError> {
    if update_columns.is_empty() {
        return Err(QueryError::BindError(
            "mysql upsert requires at least one non-conflict bind column".to_string(),
        ));
    }
    Ok(())
}

fn render_mysql_update(update_columns: &[&str]) -> Result<String, QueryError> {
    let mut sql = String::new();
    for (idx, column) in update_columns.iter().enumerate() {
        if idx > 0 {
            sql.push_str(", ");
        }
        validate_identifier(column)?;
        sql.push_str(column);
        sql.push_str(" = VALUES(");
        sql.push_str(column);
        sql.push(')');
    }
    Ok(sql)
}
