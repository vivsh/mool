//! Sealed mappings between SQL temporal expressions and Rust temporal types.

mod sealed {
    pub trait Sealed {}
}

/// A UTC-instant SQL expression type supported by portable datetime helpers.
pub trait SqlTimestamp: sealed::Sealed + 'static {
    /// Date type returned when the timestamp is converted to a date.
    type Date: 'static;
    /// Naive timestamp type returned by explicit timezone conversion.
    type Naive: 'static;
    /// Numeric extraction result, preserving nullability.
    type Part: 'static;
}

/// A non-null UTC timestamp suitable for current-time expressions.
pub trait NonNullSqlTimestamp: SqlTimestamp {}

/// A SQL date expression type supported by portable date helpers.
pub trait SqlDate: sealed::Sealed + 'static {
    /// Numeric extraction result, preserving nullability.
    type Part: 'static;
}

/// A non-null date suitable for current-date expressions.
pub trait NonNullSqlDate: SqlDate {}

/// A timezone-naive timestamp used only by explicit backend timezone helpers.
pub trait SqlNaiveTimestamp: sealed::Sealed + 'static {
    /// UTC timestamp type produced after assigning a database timezone.
    type Timestamp: 'static;
}

/// A date or timestamp accepted by date-component extraction helpers.
#[doc(hidden)]
pub trait SqlDatePartSource: sealed::Sealed + 'static {
    /// Numeric extraction result, preserving nullability.
    type Part: 'static;
}

/// A timestamp accepted by time-component extraction helpers.
#[doc(hidden)]
pub trait SqlTimePartSource: sealed::Sealed + 'static {
    /// Numeric extraction result, preserving nullability.
    type Part: 'static;
}

/// A compatible timestamp pair used by elapsed-time expressions.
#[doc(hidden)]
pub trait SqlTimestampPair<Rhs>: sealed::Sealed + 'static {
    /// Integer difference result, nullable when either input is nullable.
    type Difference: 'static;
}

macro_rules! impl_family {
    ($timestamp:ty, $date:ty, $naive:ty) => {
        impl sealed::Sealed for $timestamp {}
        impl sealed::Sealed for Option<$timestamp> {}
        impl sealed::Sealed for $date {}
        impl sealed::Sealed for Option<$date> {}
        impl sealed::Sealed for $naive {}
        impl sealed::Sealed for Option<$naive> {}

        impl SqlTimestamp for $timestamp {
            type Date = $date;
            type Naive = $naive;
            type Part = i32;
        }

        impl SqlTimestamp for Option<$timestamp> {
            type Date = Option<$date>;
            type Naive = Option<$naive>;
            type Part = Option<i32>;
        }

        impl NonNullSqlTimestamp for $timestamp {}

        impl SqlDate for $date {
            type Part = i32;
        }

        impl SqlDate for Option<$date> {
            type Part = Option<i32>;
        }

        impl NonNullSqlDate for $date {}

        impl SqlNaiveTimestamp for $naive {
            type Timestamp = $timestamp;
        }

        impl SqlNaiveTimestamp for Option<$naive> {
            type Timestamp = Option<$timestamp>;
        }

        impl SqlDatePartSource for $timestamp {
            type Part = i32;
        }

        impl SqlDatePartSource for Option<$timestamp> {
            type Part = Option<i32>;
        }

        impl SqlDatePartSource for $date {
            type Part = i32;
        }

        impl SqlDatePartSource for Option<$date> {
            type Part = Option<i32>;
        }

        impl SqlTimePartSource for $timestamp {
            type Part = i32;
        }

        impl SqlTimePartSource for Option<$timestamp> {
            type Part = Option<i32>;
        }

        impl SqlTimestampPair<$timestamp> for $timestamp {
            type Difference = i64;
        }

        impl SqlTimestampPair<Option<$timestamp>> for $timestamp {
            type Difference = Option<i64>;
        }

        impl SqlTimestampPair<$timestamp> for Option<$timestamp> {
            type Difference = Option<i64>;
        }

        impl SqlTimestampPair<Option<$timestamp>> for Option<$timestamp> {
            type Difference = Option<i64>;
        }
    };
}

impl_family!(
    chrono::DateTime<chrono::Utc>,
    chrono::NaiveDate,
    chrono::NaiveDateTime
);

#[cfg(feature = "time")]
impl_family!(time::OffsetDateTime, time::Date, time::PrimitiveDateTime);
