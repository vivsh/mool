//! SQL enum mapping support.

mod error;
mod postgres_array;
mod schema;
mod storage;
mod traits;

pub use error::SqlEnumError;
pub use schema::{SqlEnumRegistration, SqlEnumSchema, SqlSchemaBuilder, schema};
pub use storage::SqlEnumStorage;
pub use traits::SqlEnum;

#[doc(hidden)]
pub mod __private {
    pub use super::schema::{
        enum_check_name, int_check_expr, mysql_enum_type, register_enum, text_check_expr,
    };
}
