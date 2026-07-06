mod dialect;
mod error;
mod iter;
mod resolve;

#[cfg(test)]
mod tests;

pub use dialect::Dialect;
pub use error::PlaceholderError;
pub use iter::{PlaceholderIter, PlaceholderPart};
pub use resolve::{has_named_placeholder, resolve_placeholders};
