use proc_macro::TokenStream;
use proc_macro_crate::{FoundCrate, crate_name};
use quote::{format_ident, quote};

mod embedded;

#[proc_macro]
pub fn embedded_migrations(input: TokenStream) -> TokenStream {
    embedded::embedded_migrations(input, runtime_path())
}

#[proc_macro_derive(Record, attributes(column, table, db))]
pub fn derive_record(input: TokenStream) -> TokenStream {
    mool_macros_impl::record::derive_record(input.into(), runtime_path()).into()
}

#[proc_macro_derive(Model, attributes(column, table, db))]
pub fn derive_model(input: TokenStream) -> TokenStream {
    mool_macros_impl::model::derive_model(input.into(), runtime_path()).into()
}

#[proc_macro_derive(Filterable, attributes(filter, db))]
pub fn derive_filterable(input: TokenStream) -> TokenStream {
    mool_macros_impl::filterable::derive_filterable(input.into(), runtime_path()).into()
}

#[proc_macro_derive(SqlEnum, attributes(sql_enum, db))]
pub fn derive_sql_enum(input: TokenStream) -> TokenStream {
    mool_macros_impl::sql_enum::derive_sql_enum(input.into(), runtime_path()).into()
}

fn runtime_path() -> proc_macro2::TokenStream {
    match crate_name("mool") {
        Ok(FoundCrate::Itself) => quote! { ::mool },
        Ok(FoundCrate::Name(name)) => {
            let ident = format_ident!("{}", name.replace('-', "_"));
            quote! { ::#ident }
        }
        Err(_) => quote! { ::mool },
    }
}
