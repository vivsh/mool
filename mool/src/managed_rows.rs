//! Typed managed-row declarations for migration-owned configuration data.

use std::collections::BTreeMap;

use serde::Serialize;
use serde::ser::{Error as _, Serializer};
use thiserror::Error;

/// Converts a record into one migration-managed database row.
///
/// `#[derive(ManagedRecord)]` generates physical column mapping for a
/// `#[derive(Record)]` type. Implement it manually for hand-written records.
pub trait ManagedRecord: crate::Record {
    /// Returns this record's database column values for schema management.
    fn managed_values(&self) -> Result<BTreeMap<String, serde_json::Value>, ManagedRecordError>;
}

/// Failure while converting a record into migration-managed row values.
#[derive(Debug, Error)]
pub enum ManagedRecordError {
    /// A field cannot be represented as a JSON migration value.
    #[error("cannot serialize managed column '{column}': {reason}")]
    Serialize {
        /// Physical database column name.
        column: String,
        /// Gaman's detailed value conversion message.
        reason: String,
    },

    /// Flattened records emitted the same physical column more than once.
    #[error("managed record contains duplicate column '{column}'")]
    DuplicateColumn {
        /// Duplicated physical database column name.
        column: String,
    },
}

/// Serializes one record field with the same value restrictions as Gaman.
#[doc(hidden)]
pub fn managed_row_value<T>(
    value: &T,
    column: &str,
) -> Result<serde_json::Value, ManagedRecordError>
where
    T: Serialize,
{
    let source = BTreeMap::from([(column, value)]);
    let mut row =
        gaman_core::managed_rows::ManagedRow::from_serializable(&source).map_err(|reason| {
            ManagedRecordError::Serialize {
                column: column.to_string(),
                reason,
            }
        })?;
    row.values
        .remove(column)
        .map(|value| value.0)
        .ok_or_else(|| ManagedRecordError::Serialize {
            column: column.to_string(),
            reason: "Gaman did not retain the serialized column value".to_string(),
        })
}

/// Serde adapter that lets Gaman defer managed-record failures until build.
pub(crate) struct ManagedRecordAdapter<T>(pub(crate) T);

impl<T> Serialize for ManagedRecordAdapter<T>
where
    T: ManagedRecord,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0
            .managed_values()
            .map_err(S::Error::custom)?
            .serialize(serializer)
    }
}
