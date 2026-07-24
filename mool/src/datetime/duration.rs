//! Adapters from standard Rust duration values to portable SQL arithmetic.

use crate::QueryError;

mod sealed {
    pub trait Sealed {}

    impl Sealed for chrono::TimeDelta {}
    impl Sealed for std::time::Duration {}
    #[cfg(feature = "time")]
    impl Sealed for time::Duration {}
}

/// Converts an existing Rust duration into exact SQL microseconds.
///
/// Portable Mool arithmetic accepts only values exactly representable in
/// milliseconds. Implementations are sealed so every accepted type follows
/// the same precision and overflow contract.
pub trait FixedDuration: sealed::Sealed {
    #[doc(hidden)]
    fn checked_sql_microseconds(self) -> Result<i64, QueryError>;
}

impl FixedDuration for chrono::TimeDelta {
    fn checked_sql_microseconds(self) -> Result<i64, QueryError> {
        let millis = self.num_milliseconds();
        if chrono::TimeDelta::milliseconds(millis) != self {
            return Err(precision_error::<Self>());
        }
        millis.checked_mul(1_000).ok_or_else(overflow_error::<Self>)
    }
}

impl FixedDuration for std::time::Duration {
    fn checked_sql_microseconds(self) -> Result<i64, QueryError> {
        if !self.subsec_nanos().is_multiple_of(1_000_000) {
            return Err(precision_error::<Self>());
        }
        let millis = i64::try_from(self.as_millis()).map_err(|_| overflow_error::<Self>())?;
        millis.checked_mul(1_000).ok_or_else(overflow_error::<Self>)
    }
}

#[cfg(feature = "time")]
impl FixedDuration for time::Duration {
    fn checked_sql_microseconds(self) -> Result<i64, QueryError> {
        let millis = self.whole_milliseconds();
        let millis = i64::try_from(millis).map_err(|_| overflow_error::<Self>())?;
        if time::Duration::milliseconds(millis) != self {
            return Err(precision_error::<Self>());
        }
        millis.checked_mul(1_000).ok_or_else(overflow_error::<Self>)
    }
}

fn precision_error<T>() -> QueryError {
    QueryError::DateTimePrecision {
        rust_type: std::any::type_name::<T>(),
    }
}

fn overflow_error<T>() -> QueryError {
    QueryError::DateTimeOverflow {
        rust_type: std::any::type_name::<T>(),
    }
}
