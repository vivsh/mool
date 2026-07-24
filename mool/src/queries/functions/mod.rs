//! Built-in typed SQL functions and expressions.

pub mod aggregate;
#[cfg(feature = "postgres")]
pub mod arrays;
pub mod cast;
pub(crate) mod common;
pub mod datetime;
pub mod json;
#[cfg(feature = "mariadb")]
pub mod mariadb;
#[cfg(feature = "mysql")]
pub mod mysql;
#[cfg(feature = "postgres")]
pub mod postgres;
#[cfg(feature = "sqlite")]
pub mod sqlite;
