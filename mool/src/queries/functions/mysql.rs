//! MySQL-specific typed SQL helpers.

/// MySQL-specific datetime expressions.
pub mod datetime {
    pub use crate::datetime::mysql_family::*;
}
