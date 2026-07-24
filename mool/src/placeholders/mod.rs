mod dialect;
mod error;
mod iter;
mod resolve;

#[cfg(test)]
mod tests;

pub(crate) use dialect::SqlDialect as Dialect;
pub use dialect::SqlDialect;
pub use error::PlaceholderError;
pub use iter::{PlaceholderIter, PlaceholderPart};
pub use resolve::{has_named_placeholder, resolve_placeholders};
