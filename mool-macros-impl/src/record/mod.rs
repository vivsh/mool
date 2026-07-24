//! Record derive code generation.

mod codegen;

pub(super) use crate::record_types::{
    array_inner_type, is_flatten, is_json, is_reference, is_selectable, is_skip, option_inner_type,
};
pub use codegen::derive_record;
pub(crate) use codegen::derive_record_impl;
