//! MariaDB-specific typed SQL helpers.

/// MariaDB-specific datetime expressions.
pub mod datetime {
    pub use crate::datetime::mysql_family::*;
}
