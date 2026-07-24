//! Isolated database support for integration tests.

mod builder;
mod database;
mod error;
#[cfg(all(feature = "migrations", any(feature = "postgres", feature = "sqlite")))]
mod migrations;
#[cfg(any(feature = "mysql", feature = "mariadb"))]
mod mysql;
#[cfg(feature = "postgres")]
mod postgres;
#[cfg(feature = "sqlite")]
mod sqlite;
mod target;

pub use builder::{TestDatabaseBuilder, setup};
pub use database::TestDatabase;
pub use error::TestDatabaseError;

#[cfg(test)]
mod tests;
