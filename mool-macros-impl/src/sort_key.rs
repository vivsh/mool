use std::collections::HashSet;

use heck::ToSnakeCase;
use quote::quote;
use syn::{Data, DeriveInput, Error, Ident, LitInt, LitStr, Path, Variant};

/// Expands a generated request-sort vocabulary for one model-backed unit enum.
pub fn derive_sort_key(
    input: proc_macro2::TokenStream,
    runtime_path: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let input = match syn::parse2::<DeriveInput>(input) {
        Ok(input) => input,
        Err(error) => return error.to_compile_error(),
    };
    derive_sort_key_impl(&input, runtime_path).unwrap_or_else(Error::into_compile_error)
}

fn derive_sort_key_impl(
    input: &DeriveInput,
    runtime_path: proc_macro2::TokenStream,
) -> Result<proc_macro2::TokenStream, Error> {
    let Container { model, max_terms } = parse_container(input)?;
    let variants = unit_variants(input)?;
    let mappings = variants
        .iter()
        .map(parse_variant)
        .collect::<Result<Vec<_>, _>>()?;
    validate_mappings(&mappings)?;
    let crate_path = crate::runtime_path(input, runtime_path);
    let ident = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let keys = mappings.iter().map(|mapping| &mapping.name);
    let parse_arms = mappings.iter().map(|mapping| {
        let name = &mapping.name;
        let variant = &mapping.variant;
        quote! { #name => Some(Self::#variant), }
    });
    let key_arms = mappings.iter().map(|mapping| {
        let name = &mapping.name;
        let variant = &mapping.variant;
        quote! { Self::#variant => #name, }
    });
    let order_arms = mappings.iter().map(|mapping| {
        let variant = &mapping.variant;
        let column = &mapping.column;
        quote! {
            Self::#variant => match direction {
                #crate_path::SortDirection::Asc => sort.#column.asc(),
                #crate_path::SortDirection::Desc => sort.#column.desc(),
            },
        }
    });

    Ok(quote! {
        impl #impl_generics #crate_path::SortKey for #ident #ty_generics #where_clause {
            type Model = #model;
            const NAME: &'static str = stringify!(#ident);
            const MAX_TERMS: usize = #max_terms;

            fn keys() -> &'static [&'static str] {
                &[#(#keys),*]
            }

            fn parse_key(key: &str) -> Option<Self> {
                match key {
                    #(#parse_arms)*
                    _ => None,
                }
            }

            fn key(&self) -> &'static str {
                match self {
                    #(#key_arms)*
                }
            }

            fn apply_sort(
                &self,
                direction: #crate_path::SortDirection,
                sort: #crate_path::SortBuilder<Self::Model>,
            ) -> #crate_path::SortBuilder<Self::Model> {
                let order = match self {
                    #(#order_arms)*
                };
                sort.sort(order)
            }
        }
    })
}

struct Container {
    model: Path,
    max_terms: usize,
}

struct Mapping {
    variant: Ident,
    name: String,
    column: Ident,
}

fn parse_container(input: &DeriveInput) -> Result<Container, Error> {
    let attrs = input
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("sort"))
        .collect::<Vec<_>>();
    if attrs.len() != 1 {
        return Err(Error::new_spanned(
            &input.ident,
            "SortKey requires exactly one #[sort(model = ModelType)] attribute",
        ));
    }
    let mut model = None;
    let mut max_terms = None;
    attrs[0].parse_nested_meta(|meta| {
        if meta.path.is_ident("model") {
            if model.is_some() {
                return Err(meta.error("model can only be set once"));
            }
            model = Some(meta.value()?.parse::<Path>()?);
            return Ok(());
        }
        if meta.path.is_ident("max_terms") {
            if max_terms.is_some() {
                return Err(meta.error("max_terms can only be set once"));
            }
            let value = meta.value()?.parse::<LitInt>()?;
            let parsed = value
                .base10_parse::<usize>()
                .map_err(|_| meta.error("max_terms must be a positive integer"))?;
            if parsed == 0 {
                return Err(meta.error("max_terms must be greater than zero"));
            }
            max_terms = Some(parsed);
            return Ok(());
        }
        Err(meta.error("unsupported SortKey container attribute"))
    })?;
    let model = model.ok_or_else(|| Error::new_spanned(&input.ident, "SortKey requires model"))?;
    Ok(Container {
        model,
        max_terms: max_terms.unwrap_or(1),
    })
}

fn unit_variants(
    input: &DeriveInput,
) -> Result<&syn::punctuated::Punctuated<Variant, syn::Token![,]>, Error> {
    match &input.data {
        Data::Enum(data) => Ok(&data.variants),
        _ => Err(Error::new_spanned(&input.ident, "SortKey supports enums only")),
    }
}

fn parse_variant(variant: &Variant) -> Result<Mapping, Error> {
    if !matches!(variant.fields, syn::Fields::Unit) {
        return Err(Error::new_spanned(variant, "SortKey variants must be unit variants"));
    }
    let mut name = None;
    let mut column = None;
    for attr in variant.attrs.iter().filter(|attr| attr.path().is_ident("sort")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("name") {
                if name.is_some() {
                    return Err(meta.error("name can only be set once"));
                }
                name = Some(meta.value()?.parse::<LitStr>()?.value());
                return Ok(());
            }
            if meta.path.is_ident("by") {
                if column.is_some() {
                    return Err(meta.error("by can only be set once"));
                }
                column = Some(meta.value()?.parse::<Ident>()?);
                return Ok(());
            }
            Err(meta.error("unsupported SortKey variant attribute"))
        })?;
    }
    let default = variant.ident.to_string().to_snake_case();
    Ok(Mapping {
        variant: variant.ident.clone(),
        name: name.unwrap_or_else(|| default.clone()),
        column: column.unwrap_or_else(|| Ident::new(&default, variant.ident.span())),
    })
}

fn validate_mappings(mappings: &[Mapping]) -> Result<(), Error> {
    if mappings.is_empty() {
        return Err(Error::new(proc_macro2::Span::call_site(), "SortKey requires at least one variant"));
    }
    let mut names = HashSet::new();
    for mapping in mappings {
        if !valid_name(&mapping.name) {
            return Err(Error::new(mapping.variant.span(), "sort key names must be lowercase ASCII snake case"));
        }
        if !names.insert(&mapping.name) {
            return Err(Error::new(mapping.variant.span(), "sort key names must be unique"));
        }
    }
    Ok(())
}

fn valid_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    matches!(bytes.next(), Some(first) if first.is_ascii_lowercase())
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

#[cfg(test)]
mod tests {
    use super::derive_sort_key;

    /// Verifies invalid sort-key declarations return compile errors without panicking.
    #[test]
    fn invalid_sort_key_declarations_return_compile_errors() {
        let output = derive_sort_key(
            quote::quote! {
                #[derive(SortKey)]
                #[sort(model = Post, max_terms = 0)]
                enum Invalid { Title }
            },
            quote::quote!(::mool),
        )
        .to_string();

        assert!(output.contains("max_terms must be greater than zero"));
    }
}
