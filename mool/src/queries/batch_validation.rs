//! Record metadata and column validation for batch writes.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::QueryError;
use crate::interfaces::{Model, Record};

use super::batch::BatchInsertMode;
use super::expr::ColumnRef;
use super::handles::{ColumnOwner, Table};
use super::validate::validate_conflict_columns;

/// Resolves implicit upsert columns after validating the selected batch mode.
pub(super) fn resolve_batch_insert_mode<T>(
    mode: &BatchInsertMode,
    table: &Table,
    insert_columns: &[String],
) -> Result<BatchInsertMode, QueryError>
where
    T: Record,
{
    validate_batch_insert_mode::<T>(mode, table, insert_columns)?;
    let BatchInsertMode::Upsert {
        conflict,
        update_columns: None,
    } = mode
    else {
        return Ok(mode.clone());
    };
    Ok(BatchInsertMode::Upsert {
        conflict: conflict.clone(),
        update_columns: Some(default_upsert_columns::<T>(
            table,
            insert_columns,
            conflict,
        )?),
    })
}

/// Validates conflict and update selections against generated record metadata.
fn validate_batch_insert_mode<T>(
    mode: &BatchInsertMode,
    table: &Table,
    insert_columns: &[String],
) -> Result<(), QueryError>
where
    T: Record,
{
    match mode {
        BatchInsertMode::Insert => Ok(()),
        #[cfg(any(feature = "mysql", feature = "mariadb"))]
        BatchInsertMode::IgnoreErrors => Ok(()),
        #[cfg(any(feature = "postgres", feature = "sqlite"))]
        BatchInsertMode::Ignore(conflict) => validate_ignored_conflict(conflict, table),
        BatchInsertMode::Upsert {
            conflict,
            update_columns,
        } => validate_upsert::<T>(conflict, update_columns.as_deref(), table, insert_columns),
    }
}

/// Validates an optional targeted conflict-ignore clause.
#[cfg(any(feature = "postgres", feature = "sqlite"))]
fn validate_ignored_conflict(
    conflict: &Option<Vec<ColumnRef>>,
    table: &Table,
) -> Result<(), QueryError> {
    let Some(conflict) = conflict else {
        return Ok(());
    };
    if conflict.is_empty() {
        return Err(QueryError::BindError(
            "ignore_conflicts_on requires at least one column".to_string(),
        ));
    }
    validate_unique_columns(conflict, "ignored conflict target")?;
    validate_conflict_columns(conflict, table)
}

/// Validates upsert conflict and explicit update columns.
fn validate_upsert<T>(
    conflict: &[ColumnRef],
    update_columns: Option<&[ColumnRef]>,
    table: &Table,
    insert_columns: &[String],
) -> Result<(), QueryError>
where
    T: Record,
{
    if conflict.is_empty() {
        return Err(QueryError::BindError(
            "batch_upsert requires conflict columns".to_string(),
        ));
    }
    validate_unique_columns(conflict, "upsert conflict target")?;
    validate_conflict_columns(conflict, table)?;
    let Some(update_columns) = update_columns else {
        return Ok(());
    };
    validate_explicit_upsert_columns::<T>(update_columns, conflict, table, insert_columns)
}

/// Validates explicit upsert fields for ownership, uniqueness, and writability.
fn validate_explicit_upsert_columns<T>(
    columns: &[ColumnRef],
    conflict: &[ColumnRef],
    table: &Table,
    insert_columns: &[String],
) -> Result<(), QueryError>
where
    T: Record,
{
    if columns.is_empty() {
        return Err(QueryError::BindError(
            "update_only requires at least one column".to_string(),
        ));
    }
    validate_unique_columns(columns, "upsert update fields")?;
    validate_conflict_columns(columns, table)?;
    for column in columns {
        validate_upsert_column::<T>(column, conflict, insert_columns)?;
    }
    Ok(())
}

/// Validates one selected upsert field against conflict and record policies.
fn validate_upsert_column<T>(
    column: &ColumnRef,
    conflict: &[ColumnRef],
    insert_columns: &[String],
) -> Result<(), QueryError>
where
    T: Record,
{
    if conflict.iter().any(|item| item.name == column.name) {
        return Err(QueryError::BindError(format!(
            "upsert update column '{}' overlaps the conflict target",
            column.name
        )));
    }
    let insertable = insert_columns
        .iter()
        .any(|item| item == column.name.as_ref());
    let updateable = T::record_update_column_names()
        .iter()
        .any(|item| item == column.name.as_ref());
    if !insertable || !updateable {
        return Err(QueryError::BindError(format!(
            "upsert update column '{}' must be insertable and updateable",
            column.name
        )));
    }
    Ok(())
}

/// Builds default upsert fields from inserted and updateable columns.
fn default_upsert_columns<T>(
    table: &Table,
    insert_columns: &[String],
    conflict: &[ColumnRef],
) -> Result<Vec<ColumnRef>, QueryError>
where
    T: Record,
{
    let updateable = T::record_update_column_names();
    let columns = insert_columns
        .iter()
        .filter(|name| updateable.contains(name))
        .filter(|name| {
            !conflict
                .iter()
                .any(|column| column.name.as_ref() == name.as_str())
        })
        .map(|name| ColumnRef {
            owner: ColumnOwner::Root(table.clone()),
            name: Arc::from(name.as_str()),
        })
        .collect::<Vec<_>>();
    if columns.is_empty() {
        return Err(QueryError::BindError(
            "batch_upsert has no inserted, updateable non-conflict columns".to_string(),
        ));
    }
    Ok(columns)
}

/// Rejects duplicate column names in a user-provided column set.
pub(super) fn validate_unique_columns(
    columns: &[ColumnRef],
    label: &str,
) -> Result<(), QueryError> {
    let mut names = HashSet::new();
    for column in columns {
        if !names.insert(column.name.as_ref()) {
            return Err(QueryError::BindError(format!(
                "{label} contains duplicate column '{}'",
                column.name
            )));
        }
    }
    Ok(())
}

/// Ensures batch-update models and query tables refer to the same physical table.
pub(super) fn validate_model_table<T>(table: &Table) -> Result<(), QueryError>
where
    T: Model,
{
    let expected = T::table_schema()
        .map(|schema| format!("{schema}.{}", T::table_name()))
        .unwrap_or_else(|| T::table_name().to_string());
    let got = table
        .data
        .schema
        .as_deref()
        .map(|schema| format!("{schema}.{}", table.data.name))
        .unwrap_or_else(|| table.data.name.to_string());
    if expected != got {
        return Err(QueryError::TableMismatch { expected, got });
    }
    Ok(())
}

/// Validates batch-update columns against primary-key and update metadata.
pub(super) fn validate_batch_update_columns<T>(
    columns: &[ColumnRef],
    primary_keys: &[&str],
) -> Result<(), QueryError>
where
    T: Model,
{
    if columns.is_empty() {
        return Err(QueryError::BindError(
            "batch_update requires at least one update column".to_string(),
        ));
    }
    let writable = T::record_update_column_names();
    for column in columns {
        if primary_keys.iter().any(|key| *key == column.name.as_ref()) {
            return Err(QueryError::BindError(format!(
                "batch_update cannot change primary-key column '{}'",
                column.name
            )));
        }
        if !writable.iter().any(|name| name == column.name.as_ref()) {
            return Err(QueryError::BindError(format!(
                "batch update column '{}' is not writable",
                column.name
            )));
        }
    }
    Ok(())
}

/// Rejects duplicate primary-key values before a batch-update session call.
pub(super) fn validate_batch_update_keys<T>(rows: &[T]) -> Result<(), QueryError>
where
    T: Model,
{
    let mut keys = HashMap::with_capacity(rows.len());
    for (index, row) in rows.iter().enumerate() {
        if let Some(first) = keys.insert(row.primary_key(), index) {
            return Err(QueryError::DuplicateBatchKey {
                first,
                duplicate: index,
            });
        }
    }
    Ok(())
}
