//! Schema helpers for SQL enum mappings.

use crate::Model;
use crate::schema::{FunctionDef, IntoTable, Schema, SchemaBuilder, SchemaLoadError};
use gaman::core::Dialect;

use super::{SqlEnum, SqlEnumStorage};

/// Function pointer used by generated model metadata to register enum types.
#[derive(Clone, Copy)]
pub struct SqlEnumRegistration {
    register: fn(SqlSchemaBuilder) -> SqlSchemaBuilder,
}

impl SqlEnumRegistration {
    /// Creates a registration callback for one SQL enum type.
    pub const fn new(register: fn(SqlSchemaBuilder) -> SqlSchemaBuilder) -> Self {
        Self { register }
    }

    fn apply(self, builder: SqlSchemaBuilder) -> SqlSchemaBuilder {
        (self.register)(builder)
    }
}

/// Schema metadata supplied by generated model impls for SQL enum fields.
pub trait SqlEnumSchema {
    /// Enum types used by this model.
    const SQL_ENUMS: &'static [SqlEnumRegistration];
}

/// Creates an enum-aware schema builder for the selected backend.
pub fn schema() -> SqlSchemaBuilder {
    SqlSchemaBuilder::new()
}

/// Enum-aware wrapper around Gaman schema building.
pub struct SqlSchemaBuilder {
    dialect: Dialect,
    inner: SchemaBuilder,
}

impl Default for SqlSchemaBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl SqlSchemaBuilder {
    /// Creates a schema builder for the selected backend.
    pub fn new() -> Self {
        let dialect = crate::backend::gaman_dialect();
        Self {
            dialect,
            inner: SchemaBuilder::new(dialect),
        }
    }

    /// Adds a table from any Gaman-compatible table type.
    pub fn table<T>(mut self) -> Self
    where
        T: IntoTable,
    {
        self.inner = self.inner.table::<T>();
        self
    }

    /// Adds a model table and its native enum definitions.
    pub fn model<T>(mut self) -> Self
    where
        T: Model + SqlEnumSchema,
    {
        for registration in T::SQL_ENUMS {
            self = registration.apply(self);
        }
        self.table::<T>()
    }

    /// Adds a native enum type when the current dialect can manage it.
    pub fn enum_type<E>(mut self) -> Self
    where
        E: SqlEnum,
    {
        if matches!(
            (self.dialect, E::SQL_STORAGE),
            (Dialect::Postgres, SqlEnumStorage::NativePostgres)
        ) {
            self.inner = self.inner.enum_type(E::SQL_NAME, E::SQL_VALUES);
        }
        self
    }

    /// Adds a database extension.
    pub fn extension(mut self, name: impl Into<String>) -> Self {
        self.inner = self.inner.extension(name);
        self
    }

    /// Adds a database view definition.
    pub fn view(mut self, name: impl Into<String>, definition: impl Into<String>) -> Self {
        self.inner = self.inner.view(name, definition);
        self
    }

    /// Adds a database function definition.
    pub fn function(mut self, function: FunctionDef) -> Self {
        self.inner = self.inner.function(function);
        self
    }

    /// Builds validated schema metadata for the selected dialect.
    ///
    /// Returns a schema-loading error when the assembled metadata is invalid
    /// for that dialect.
    pub fn build(self) -> Result<Schema, SchemaLoadError> {
        self.inner.build()
    }
}

/// Registers one enum type with an enum-aware schema builder.
pub fn register_enum<E>(builder: SqlSchemaBuilder) -> SqlSchemaBuilder
where
    E: SqlEnum,
{
    builder.enum_type::<E>()
}

/// Returns a deterministic table-check name for one enum column.
pub fn enum_check_name(table: &str, column: &str) -> String {
    format!("ck_{table}_{column}_sql_enum")
}

/// Returns a SQL text-label check expression.
pub fn text_check_expr(column: &str, values: &[&str]) -> String {
    let values = values
        .iter()
        .map(|value| quote_sql_string(value))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{column} IN ({values})")
}

/// Returns a SQL integer-code check expression.
pub fn int_check_expr<T>(column: &str, values: &[T]) -> String
where
    T: std::fmt::Display,
{
    let values = values
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    format!("{column} IN ({values})")
}

/// Returns a MySQL `ENUM(...)` column type.
pub fn mysql_enum_type(values: &[&str]) -> String {
    let values = values
        .iter()
        .map(|value| quote_sql_string(value))
        .collect::<Vec<_>>()
        .join(", ");
    format!("ENUM({values})")
}

fn quote_sql_string(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}
