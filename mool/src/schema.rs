//! Model-aware schema metadata backed by Gaman.

pub use gaman::schema::{
    Column, ColumnBuilder, ColumnDesc, ColumnRef, ColumnType, Constraint, ConstraintInput, EnumDef,
    EnumInput, ExtensionDef, ExtensionInput, ForeignKey, FunctionDef, FunctionInput,
    GeneratedStorage, Index, IndexInput, InputSchema, IntoTable, PrimaryKey, Schema, SchemaBuilder,
    SchemaLoadError, SchemaValidationError, Table, TableBuilder, TableInput, TriggerDef,
    TriggerEvent, TriggerInput, TriggerScope, TriggerTiming, ViewDef, ViewInput, Volatility,
};

pub use crate::enums::{SqlSchemaBuilder, schema};
