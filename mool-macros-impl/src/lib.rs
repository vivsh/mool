//! Internal derive implementation shared by Mool and framework wrappers.
//!
//! This crate is public only so proc-macro crates can reuse one implementation.
//! Application code should depend on `mool`, not this crate.

pub mod filterable;
pub mod model;
pub mod record;
mod schemable;
pub mod sql_enum;
mod typed_handles;

use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, LitStr};

/// Resolves the runtime path used in generated DB code.
pub fn runtime_path(input: &DeriveInput, default_path: TokenStream) -> TokenStream {
    match db_crate_override(input) {
        Ok(Some(path)) => path,
        Ok(None) => default_path,
        Err(err) => err.to_compile_error(),
    }
}

fn db_crate_override(input: &DeriveInput) -> syn::Result<Option<TokenStream>> {
    let mut out = None;
    for attr in input.attrs.iter().filter(|attr| attr.path().is_ident("db")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("crate") {
                let value = meta.value()?;
                let path: LitStr = value.parse()?;
                out = Some(path.parse()?);
                return Ok(());
            }
            Err(meta.error("unsupported db attribute"))
        })?;
    }
    Ok(out)
}

/// Default path used by wrapper crates that intentionally pass none.
pub fn default_mool_path() -> TokenStream {
    quote! { ::mool }
}
