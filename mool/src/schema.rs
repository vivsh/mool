//! Model-aware schema metadata backed by Gaman.
//!
//! [`SchemaBuilder`] is Mool's selected-dialect builder for model and enum
//! metadata. Gaman schema values remain available for interoperability, but
//! its builder is intentionally not re-exported.

pub use gaman::schema::{
    Column, ColumnBuilder, ColumnDesc, ColumnRef, ColumnType, Constraint, ConstraintInput, EnumDef,
    EnumInput, ExtensionDef, ExtensionInput, ForeignKey, FunctionDef, FunctionInput,
    GeneratedStorage, Index, IndexInput, InputSchema, IntoTable, PrimaryKey, Schema,
    SchemaLoadError, SchemaValidationError, Table, TableBuilder, TableInput, TriggerDef,
    TriggerEvent, TriggerInput, TriggerScope, TriggerTiming, ViewDef, ViewInput, Volatility,
};

#[cfg(mool_has_backend)]
pub use crate::enums::{SchemaBuilder, schema};

#[cfg(mool_has_backend)]
#[doc(hidden)]
pub mod __private;
