//! SQL enum mapping support.

mod error;
mod postgres_array;
#[cfg(mool_has_backend)]
mod schema;
mod storage;
mod traits;

pub use error::SqlEnumError;
#[cfg(mool_has_backend)]
pub use schema::{SqlEnumRegistration, SqlEnumSchema, SqlSchemaBuilder, schema};
pub use storage::SqlEnumStorage;
pub use traits::SqlEnum;

#[doc(hidden)]
#[cfg(mool_has_backend)]
pub mod __private {
    pub use super::schema::{
        enum_check_name, int_check_expr, mysql_enum_type, register_enum, text_check_expr,
    };
}
