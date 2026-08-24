//! Shared model and record attribute parsing.

mod parser;
#[cfg(any(feature = "text-search", feature = "fuzzy-search", feature = "vector"))]
mod search;

pub use parser::*;
#[cfg(feature = "vector")]
pub use search::{VectorSpec, is_vector_type};
