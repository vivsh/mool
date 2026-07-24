//! Temporal type adapters used by Mool's typed datetime expressions.

mod duration;
mod traits;

#[cfg(any(feature = "mysql", feature = "mariadb"))]
pub(crate) mod mysql_family;
pub(crate) mod portable;
#[cfg(feature = "postgres")]
pub(crate) mod postgres;
#[cfg(feature = "sqlite")]
pub(crate) mod sqlite;

pub use duration::FixedDuration;
pub use traits::{
    NonNullSqlDate, NonNullSqlTimestamp, SqlDate, SqlDatePartSource, SqlNaiveTimestamp,
    SqlTimePartSource, SqlTimestamp, SqlTimestampPair,
};
