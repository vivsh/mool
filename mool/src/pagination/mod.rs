//! Request-side pagination inputs and result envelopes.

mod builder;
mod input;
mod response;
mod traits;

pub use builder::PaginationBuilder;
pub use input::Pagination;
pub use response::Page;
pub use traits::Pageable;
