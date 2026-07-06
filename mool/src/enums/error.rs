//! Errors raised while decoding SQL enum values.

/// Error returned when a database value does not match a generated SQL enum.
#[derive(Debug, thiserror::Error)]
pub enum SqlEnumError {
    /// A text-backed enum label was not one of the known variants.
    #[error("unknown SQL enum label '{value}' for {enum_name}")]
    UnknownLabel {
        /// SQL enum type name.
        enum_name: &'static str,
        /// Database label that failed to decode.
        value: String,
    },

    /// An integer-backed enum code was not one of the known variants.
    #[error("unknown SQL enum code '{value}' for {enum_name}")]
    UnknownCode {
        /// SQL enum type name.
        enum_name: &'static str,
        /// Database code that failed to decode.
        value: i64,
    },
}
