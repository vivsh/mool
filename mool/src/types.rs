//! Database expression marker types.

use std::marker::PhantomData;

/// Marker for SQL JSON/JSONB expressions in typed queries.
///
/// This is not a Rust value wrapper. Model fields keep their actual Rust type,
/// while generated query columns use this marker when the column is stored as
/// JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Json;

/// Marker for SQL array expressions in typed queries.
///
/// This is not a Rust value wrapper. Model fields keep using `Vec<T>` or
/// `Option<Vec<T>>`, while generated query columns use this marker for SQL
/// array expressions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Array<T>(PhantomData<fn() -> T>);
