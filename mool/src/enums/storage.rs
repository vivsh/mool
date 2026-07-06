//! Storage modes for generated SQL enum mappings.

/// Database representation used by a [`crate::SqlEnum`] mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SqlEnumStorage {
    /// Store enum variants as text labels.
    Text,
    /// Store enum variants as explicit signed integer codes.
    Int,
    /// Store enum variants in a PostgreSQL native enum type.
    NativePostgres,
    /// Store enum variants in a MySQL native `ENUM(...)` column type.
    NativeMysql,
}
