//! `SqlEnum` derive implementation.

mod attrs;
mod codegen;
mod model;
mod rename;

#[cfg(test)]
mod tests;

use syn::DeriveInput;

pub fn derive_sql_enum(
    input: proc_macro2::TokenStream,
    runtime_path: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let input = match syn::parse2::<DeriveInput>(input) {
        Ok(input) => input,
        Err(err) => return err.to_compile_error(),
    };
    let crate_path = crate::runtime_path(&input, runtime_path);
    match model::ParsedSqlEnum::from_input(&input) {
        Ok(parsed) => codegen::generate(&parsed, &crate_path),
        Err(err) => err.to_compile_error(),
    }
}
