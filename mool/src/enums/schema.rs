//! Selected-dialect schema construction for models and authored metadata.

use crate::managed_rows::ManagedRecordAdapter;
use crate::schema::{
    FunctionDef, IntoTable, Schema, SchemaLoadError, SchemaValidationError, Table, TableBuilder,
};
use crate::{ManagedRecord, Model, NoPrefix, TablePrefix};
use gaman::core::Dialect;
use gaman::schema::SchemaBuilder as GamanSchemaBuilder;

use super::{SqlEnum, SqlEnumStorage};

/// Function pointer used by generated model metadata to register enum types.
#[derive(Clone, Copy)]
pub struct SqlEnumRegistration {
    register: fn(SchemaBuilder) -> SchemaBuilder,
}

impl SqlEnumRegistration {
    /// Creates a registration callback for one SQL enum type.
    pub const fn new(register: fn(SchemaBuilder) -> SchemaBuilder) -> Self {
        Self { register }
    }

    fn apply(self, builder: SchemaBuilder) -> SchemaBuilder {
        (self.register)(builder)
    }
}

/// Schema metadata supplied by generated model impls for SQL enum fields.
pub trait SqlEnumSchema {
    /// Enum types used by this model.
    const SQL_ENUMS: &'static [SqlEnumRegistration];
}

/// Creates a model-aware schema builder for the selected backend.
pub fn schema() -> SchemaBuilder {
    SchemaBuilder::new()
}

/// Mool's model-aware wrapper around Gaman schema construction.
pub struct SchemaBuilder {
    dialect: Dialect,
    inner: GamanSchemaBuilder,
    extensions: std::collections::HashSet<String>,
    issues: Vec<String>,
}

impl Default for SchemaBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl SchemaBuilder {
    /// Creates a schema builder for the selected backend.
    pub fn new() -> Self {
        let dialect = crate::backend::gaman_dialect();
        Self {
            dialect,
            inner: GamanSchemaBuilder::new(dialect),
            extensions: std::collections::HashSet::new(),
            issues: Vec::new(),
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
    pub fn model<T>(self) -> Self
    where
        T: Model + SqlEnumSchema,
    {
        self.model_with::<NoPrefix, T>()
    }

    /// Adds a prefixed model table and its native enum definitions.
    ///
    /// Only the registered table name is prefixed. Relation and foreign-key
    /// targets retain the model's declared metadata.
    pub fn model_with<P, T>(mut self) -> Self
    where
        P: TablePrefix,
        T: Model + SqlEnumSchema,
    {
        for registration in T::SQL_ENUMS {
            self = registration.apply(self);
        }
        let table = prefixed_table::<P, T>(&self.dialect);
        if let Err(error) = crate::queries::__private::validate_identifier(&table.name) {
            self.issues
                .push(format!("{}: {error}", table.qualified_name()));
        }
        #[cfg(any(feature = "text-search", feature = "fuzzy-search", feature = "vector"))]
        {
            let model = T::model_schema();
            if let Err(error) =
                crate::search::__private::validate_dialect(&model.search, self.dialect)
            {
                self.issues
                    .push(format!("{}: {error}", table.qualified_name()));
            }
            for extension in
                crate::search::__private::required_extensions(&model.search, self.dialect)
            {
                self = self.extension(extension);
            }
            for statement in crate::search::__private::index_statements(
                &table.name,
                table.schema.as_deref(),
                &model.search,
                &self.dialect,
            ) {
                self = self.opaque(statement);
            }
        }
        self.inner = self.inner.table_def(table);
        self
    }

    /// Adds the complete desired managed-row set for one registered model table.
    ///
    /// The target model must be added through [`Self::model`] or [`Self::table`]
    /// in the same builder. Gaman validates the target, keys, and row values at
    /// [`Self::build`].
    pub fn managed_rows<M>(self, rows: impl IntoIterator<Item = impl ManagedRecord>) -> Self
    where
        M: Model,
    {
        self.managed_rows_with::<NoPrefix, M>(rows)
    }

    /// Adds managed rows for a model table registered with `P`'s prefix.
    pub fn managed_rows_with<P, M>(
        mut self,
        rows: impl IntoIterator<Item = impl ManagedRecord>,
    ) -> Self
    where
        P: TablePrefix,
        M: Model,
    {
        let table = prefixed_table::<P, M>(&self.dialect).qualified_name();
        self.inner = self
            .inner
            .managed_rows(table, rows.into_iter().map(ManagedRecordAdapter));
        self
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
        let name = name.into();
        if self.extensions.insert(name.clone()) {
            self.inner = self.inner.extension(name);
        }
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

    /// Adds a verified opaque `CREATE` declaration.
    ///
    /// Gaman classifies the declaration and preserves its SQL for migration
    /// diffing and replay. Invalid or ambiguous declarations fail at [`Self::build`].
    pub fn opaque(mut self, create_sql: impl Into<String>) -> Self {
        self.inner = self.inner.opaque(create_sql);
        self
    }

    /// Additively configures an existing table without changing its identity.
    ///
    /// The callback can add modeled fields as well as unmanaged prefixes,
    /// suffixes, and named constraints. Invalid identities fail at [`Self::build`].
    pub fn extend_table(
        mut self,
        name: impl Into<String>,
        configure: impl FnOnce(TableBuilder) -> TableBuilder,
    ) -> Self {
        self.inner = self.inner.extend_table(name, configure);
        self
    }

    /// Builds validated schema metadata for the selected dialect.
    ///
    /// Returns a schema-loading error when the assembled metadata is invalid
    /// for that dialect.
    pub fn build(self) -> Result<Schema, SchemaLoadError> {
        if !self.issues.is_empty() {
            return Err(SchemaLoadError::Validation(SchemaValidationError::Invalid(
                self.issues.join("; "),
            )));
        }
        self.inner.build()
    }
}

/// Produces one model's desired table under a compile-time prefix.
fn prefixed_table<P, M>(dialect: &Dialect) -> Table
where
    P: TablePrefix,
    M: Model,
{
    let mut table = M::into_table(dialect);
    table.name = P::table_name::<M>();
    table
}

/// Registers one enum type with an enum-aware schema builder.
pub fn register_enum<E>(builder: SchemaBuilder) -> SchemaBuilder
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
