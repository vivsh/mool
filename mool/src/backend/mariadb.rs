//! MariaDB backend identity.

/// Name of the selected backend.
pub const NAME: &str = "mariadb";

/// Maximum number of placeholders used by one Mool MariaDB batch statement.
pub const PARAMETER_LIMIT: usize = 65_535;
