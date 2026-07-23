use std::sync::Arc;

use super::{Dialect, PlaceholderError, PlaceholderIter, PlaceholderPart};

/// Check if SQL string contains at least one named placeholder (:name).
/// Returns true on first match without scanning the entire string.
pub fn has_named_placeholder(sql: &str) -> bool {
    PlaceholderIter::new(sql).any(|part| matches!(part, PlaceholderPart::Placeholder(_)))
}

/// Resolve named placeholders (:name) to database-specific format.
/// Binds values from the map to the arguments and returns the transformed SQL.
/// Returns an error if a placeholder is not found in the map.
/// For PostgreSQL, reuses positions for placeholders that appear multiple times.
pub fn resolve_placeholders(
    sql: &str,
    arguments: &mut crate::commons::Arguments<'static>,
    values: &std::collections::HashMap<String, crate::argvalue::ArgValue>,
    dialect: Dialect,
) -> Result<String, PlaceholderError> {
    use sqlx::Arguments as _;

    let mut output = String::with_capacity(sql.len());
    let mut position = arguments.len() + 1;

    // Only needed for Postgres reuse
    let mut bound_positions: std::collections::HashMap<&str, usize> =
        std::collections::HashMap::new();

    for part in PlaceholderIter::new(sql) {
        match part {
            PlaceholderPart::Sql(s) => output.push_str(s),
            PlaceholderPart::Placeholder(name) => {
                match dialect {
                    Dialect::Postgres => {
                        let pos = if let Some(&p) = bound_positions.get(name) {
                            p
                        } else {
                            let value = values
                                .get(name)
                                .ok_or_else(|| PlaceholderError::MissingValue(name.to_string()))?;
                            value.bind_value(arguments).map_err(|e| {
                                PlaceholderError::BindError {
                                    placeholder: name.to_string(),
                                    source: Arc::from(e)
                                        as Arc<dyn std::error::Error + Send + Sync>,
                                }
                            })?;
                            let p = position;
                            bound_positions.insert(name, p);
                            position += 1;
                            p
                        };

                        output.push('$');
                        output.push_str(&pos.to_string()); // swap to itoa if you care
                    }

                    Dialect::Mysql | Dialect::Mariadb | Dialect::Sqlite => {
                        let value = values
                            .get(name)
                            .ok_or_else(|| PlaceholderError::MissingValue(name.to_string()))?;
                        value
                            .bind_value(arguments)
                            .map_err(|e| PlaceholderError::BindError {
                                placeholder: name.to_string(),
                                source: Arc::from(e) as Arc<dyn std::error::Error + Send + Sync>,
                            })?;
                        position += 1;
                        output.push('?');
                    }
                }
            }
        }
    }

    Ok(output)
}
