use std::hash::Hash;
use std::marker::PhantomData;

use crate::backend::{Arguments, Row};
use crate::relations::ReferenceMeta;

/// Fluent metadata descriptor for a [`Record`].
///
/// This is the pure-Rust surface that `#[derive(Record)]` delegates to. The
/// same record can be described by hand without the macro by implementing
/// [`Record::record_schema`] with this builder; only the type-directed
/// scan/bind methods then need to be provided.
#[derive(Clone, Debug)]
pub struct RecordSchema<T> {
    pub(crate) table_name: &'static str,
    pub(crate) table_schema: Option<&'static str>,
    pub(crate) root_name: Option<&'static str>,
    pub(crate) references: Vec<ReferenceMeta>,
    pub(crate) column_names: Vec<String>,
    pub(crate) bind_column_names: Vec<String>,
    _marker: PhantomData<fn() -> T>,
}

impl<T> RecordSchema<T> {
    /// Starts a record schema rooted at `table_name`.
    pub fn new(table_name: &'static str) -> Self {
        Self {
            table_name,
            table_schema: None,
            root_name: None,
            references: Vec::new(),
            column_names: Vec::new(),
            bind_column_names: Vec::new(),
            _marker: PhantomData,
        }
    }

    /// Sets the optional table schema.
    pub fn schema(mut self, schema: impl Into<Option<&'static str>>) -> Self {
        self.table_schema = schema.into();
        self
    }

    /// Sets the logical root alias used when scanning from a joined query.
    pub fn root(mut self, root: impl Into<Option<&'static str>>) -> Self {
        self.root_name = root.into();
        self
    }

    /// Appends a single joined reference.
    pub fn reference(mut self, reference: ReferenceMeta) -> Self {
        self.references.push(reference);
        self
    }

    /// Replaces the joined references.
    pub fn references(mut self, references: Vec<ReferenceMeta>) -> Self {
        self.references = references;
        self
    }

    /// Appends a single scan column name.
    pub fn column(mut self, name: impl Into<String>) -> Self {
        self.column_names.push(name.into());
        self
    }

    /// Replaces the scan column names.
    pub fn columns(mut self, columns: Vec<String>) -> Self {
        self.column_names = columns;
        self
    }

    /// Appends a single bind column name.
    pub fn bind_column(mut self, name: impl Into<String>) -> Self {
        self.bind_column_names.push(name.into());
        self
    }

    /// Replaces the bind column names.
    pub fn bind_columns(mut self, columns: Vec<String>) -> Self {
        self.bind_column_names = columns;
        self
    }
}

/// Fluent metadata descriptor for a [`Model`].
///
/// Wraps a [`RecordSchema`] and adds the primary-key column identity so a model
/// can be described by hand without the derive macro.
#[derive(Clone, Debug)]
pub struct ModelSchema<T> {
    pub(crate) record: RecordSchema<T>,
    pub(crate) primary_key_columns: &'static [&'static str],
}

impl<T> ModelSchema<T> {
    /// Builds a model schema from a record schema and its primary-key columns.
    pub fn new(record: RecordSchema<T>, primary_key_columns: &'static [&'static str]) -> Self {
        Self {
            record,
            primary_key_columns,
        }
    }

    /// The underlying record metadata for this model.
    pub fn record(&self) -> &RecordSchema<T> {
        &self.record
    }

    /// Primary-key column names in declaration order.
    pub fn primary_key_columns(&self) -> &'static [&'static str] {
        self.primary_key_columns
    }
}

/// Reusable database row/value shape.
///
/// A record describes columns that can be scanned from query results and bound
/// into insert or update statements. Table-backed records should derive
/// [`Model`] to add table identity and primary-key metadata.
///
/// The only required method is [`Record::record_schema`]; all metadata accessors
/// default to reading it, so the derive macro stays thin and a hand-written
/// record only supplies the schema plus its scan/bind behavior.
pub trait Record: Sized {
    /// Fluent metadata descriptor for this record.
    fn record_schema() -> RecordSchema<Self>;

    /// Root table name used when this record is selected.
    fn record_table_name() -> &'static str {
        Self::record_schema().table_name
    }

    /// Optional root schema used when this record is selected.
    fn record_table_schema() -> Option<&'static str> {
        Self::record_schema().table_schema
    }

    /// Logical root name used when this record is scanned from a joined query.
    fn record_root_name() -> Option<&'static str> {
        Self::record_schema().root_name
    }

    /// References that must be joined when this record is selected.
    fn record_references() -> Vec<ReferenceMeta> {
        Self::record_schema().references
    }

    /// Columns read by this record in scan order.
    fn record_column_names() -> Vec<String> {
        Self::record_schema().column_names
    }

    /// Columns written by this record in bind order.
    fn record_bind_column_names() -> Vec<String> {
        Self::record_schema().bind_column_names
    }

    /// Bind this record's writable values into SQL arguments.
    fn record_bind_values(&self, _args: &mut Arguments<'static>) -> Result<(), sqlx::Error> {
        Ok(())
    }

    /// Bind only the requested writable columns in the provided order.
    ///
    /// The default implementation supports the full bind-column list. Derived
    /// records override this so expression-based writes can replace individual
    /// fields without binding unused values.
    fn record_bind_selected(
        &self,
        columns: &[&str],
        args: &mut Arguments<'static>,
    ) -> Result<(), sqlx::Error> {
        let expected = Self::record_bind_column_names();
        if columns.len() != expected.len() {
            return Err(sqlx::Error::Protocol(
                "record does not support selective binding".to_string(),
            ));
        }
        if columns.iter().zip(expected.iter()).all(|(a, b)| *a == b) {
            return self.record_bind_values(args);
        }
        Err(sqlx::Error::Protocol(
            "record does not support selective binding".to_string(),
        ))
    }

    /// Scan this record from the current ordered row position.
    fn record_scan_ordered(_row: &Row, _start_idx: &mut usize) -> Result<Self, sqlx::Error> {
        Err(sqlx::Error::Protocol(
            "record does not support row scanning".to_string(),
        ))
    }

    /// Scan this record by column names.
    fn record_scan_unordered(_row: &Row) -> Result<Self, sqlx::Error> {
        Err(sqlx::Error::Protocol(
            "record does not support unordered row scanning".to_string(),
        ))
    }

    /// Scan this record from the start of a row.
    fn record_scan(row: &Row) -> Result<Self, sqlx::Error> {
        let mut idx = 0;
        Self::record_scan_ordered(row, &mut idx)
    }
}

/// Columnar representation generated for efficient backend batch strategies.
///
/// Derived records transpose writable fields into typed vectors without
/// referring to a database backend. Manual records may implement this trait to
/// opt into strategies such as PostgreSQL `UNNEST`.
pub trait BatchRecord: Record {
    /// Nested tuple of writable column vectors in record bind order.
    type BatchColumns;

    /// Transposes row-oriented values into writable column vectors.
    fn batch_columns(rows: &[Self]) -> Result<Self::BatchColumns, sqlx::Error>;
}

/// Table-backed database record with schema and primary-key metadata.
pub trait Model: Record + crate::schema::IntoTable {
    type PrimaryKey: Clone + Hash + Eq;

    /// Fluent model metadata descriptor.
    fn model_schema() -> ModelSchema<Self>
    where
        Self: Sized;

    /// Database table name for this model.
    fn table_name() -> &'static str {
        Self::record_table_name()
    }

    /// Optional database schema for this model table.
    fn table_schema() -> Option<&'static str> {
        Self::record_table_schema()
    }

    /// Typed table handle used by the source-first query API.
    fn table() -> crate::queries::__private::ModelTable<Self>
    where
        Self: Sized + crate::queries::__private::HasCols,
    {
        let table = match Self::table_schema() {
            Some(schema) => crate::queries::__private::table_schema(schema, Self::table_name()),
            None => crate::queries::__private::table(Self::table_name()),
        };
        crate::queries::__private::ModelTable::new_with_columns(table, Self::record_column_names())
    }

    /// Return this row's primary-key value.
    fn primary_key(&self) -> Self::PrimaryKey;

    /// Primary-key column names in declaration order.
    fn primary_key_columns() -> &'static [&'static str]
    where
        Self: Sized,
    {
        Self::model_schema().primary_key_columns
    }

    /// Single primary-key column name when the model uses a one-column key.
    fn primary_key_column() -> Option<&'static str>
    where
        Self: Sized,
    {
        match Self::primary_key_columns() {
            [column] => Some(*column),
            _ => None,
        }
    }
}
