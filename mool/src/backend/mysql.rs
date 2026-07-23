//! MySQL backend identity.

/// Name of the selected backend.
pub const NAME: &str = "mysql";

/// Maximum number of placeholders used by one Mool MySQL batch statement.
pub const PARAMETER_LIMIT: usize = 65_535;
