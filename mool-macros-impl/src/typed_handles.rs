use quote::{format_ident, quote};

use crate::record::{
    array_inner_type, is_flatten, is_json, is_reference, is_selectable, is_skip, option_inner_type,
};
use crate::schemable::{FieldMeta, ParsedStruct};

pub(super) fn gen_typed_handles(
    parsed: &ParsedStruct,
    crate_path: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let ident = &parsed.ident;
    let cols_ident = format_ident!("{}Cols", ident);
    let projected_ident = format_ident!("{}ProjectedCols", ident);
    let output_ident = format_ident!("{}OutputCols", ident);
    let generics = parsed.generics.clone();
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let cols_fields = gen_cols_fields(&parsed.fields, crate_path, false);
    let table_inits = gen_cols_inits(&parsed.fields, crate_path, false, false);
    let reference_inits = gen_cols_inits(&parsed.fields, crate_path, false, true);
    let projected_fields = gen_cols_fields(&parsed.fields, crate_path, true);
    let projected_inits = gen_cols_inits(&parsed.fields, crate_path, true, false);
    let output_fields = gen_output_fields(&parsed.fields, crate_path);
    let output_inits = gen_output_inits(&parsed.fields, crate_path);

    quote! {
        #[derive(Clone)]
        pub struct #cols_ident #impl_generics #where_clause {
            #(#cols_fields)*
        }

        #[derive(Clone)]
        pub struct #projected_ident #impl_generics #where_clause {
            #(#projected_fields)*
        }

        #[derive(Clone)]
        pub struct #output_ident #impl_generics #where_clause {
            #(#output_fields)*
        }

        impl #impl_generics #crate_path::queries::__private::HasCols for #ident #ty_generics #where_clause {
            type Columns = #cols_ident #ty_generics;

            fn cols_for_table(table: &#crate_path::queries::__private::Table) -> Self::Columns {
                #cols_ident {
                    #(#table_inits)*
                }
            }

            fn cols_for_reference(reference: &#crate_path::queries::__private::Reference) -> Self::Columns {
                #cols_ident {
                    #(#reference_inits)*
                }
            }
        }

        impl #impl_generics #crate_path::queries::__private::Projectable for #ident #ty_generics #where_clause {
            type Columns = #projected_ident #ty_generics;

            fn projected_columns(
                source: #crate_path::queries::__private::ProjectionSource,
            ) -> Self::Columns {
                #projected_ident {
                    #(#projected_inits)*
                }
            }
        }

        impl #impl_generics #crate_path::queries::__private::HasOutputCols for #ident #ty_generics #where_clause {
            type OutputColumns = #output_ident #ty_generics;

            fn output_columns(
                source: #crate_path::queries::__private::OutputSource,
            ) -> Self::OutputColumns {
                #output_ident {
                    #(#output_inits)*
                }
            }
        }
    }
}

fn gen_output_fields(
    fields: &[FieldMeta],
    crate_path: &proc_macro2::TokenStream,
) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .filter(|field| !is_skip(field) && is_selectable(field))
        .filter_map(|field| gen_output_field(field, crate_path))
        .collect()
}

fn gen_output_field(
    field: &FieldMeta,
    crate_path: &proc_macro2::TokenStream,
) -> Option<proc_macro2::TokenStream> {
    let ident = field.ident.as_ref()?;
    if is_flatten(field) || is_reference(field) {
        let ty = option_inner_type(&field.ty).unwrap_or(&field.ty);
        return Some(quote! {
            pub #ident: <#ty as #crate_path::queries::__private::HasOutputCols>::OutputColumns,
        });
    }
    let ty = column_expr_ty(field, crate_path);
    Some(quote! {
        pub #ident: #crate_path::queries::__private::OutputColumn<#ty>,
    })
}

fn gen_output_inits(
    fields: &[FieldMeta],
    crate_path: &proc_macro2::TokenStream,
) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .filter(|field| !is_skip(field) && is_selectable(field))
        .filter_map(|field| gen_output_init(field, crate_path))
        .collect()
}

fn gen_output_init(
    field: &FieldMeta,
    crate_path: &proc_macro2::TokenStream,
) -> Option<proc_macro2::TokenStream> {
    let ident = field.ident.as_ref()?;
    if is_flatten(field) {
        let ty = &field.ty;
        return Some(quote! {
            #ident: <#ty as #crate_path::queries::__private::HasOutputCols>::output_columns(source.clone()),
        });
    }
    if is_reference(field) {
        let ty = option_inner_type(&field.ty).unwrap_or(&field.ty);
        let name = column_name(field);
        return Some(quote! {
            #ident: <#ty as #crate_path::queries::__private::HasOutputCols>::output_columns(source.nested(#name)),
        });
    }
    let name = column_name(field);
    Some(quote! { #ident: source.col(#name), })
}

fn gen_cols_fields(
    fields: &[FieldMeta],
    crate_path: &proc_macro2::TokenStream,
    projected: bool,
) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .filter(|field| !is_skip(field) && is_selectable(field))
        .filter_map(|field| gen_cols_field(field, crate_path, projected))
        .collect()
}

fn gen_cols_field(
    field: &FieldMeta,
    crate_path: &proc_macro2::TokenStream,
    projected: bool,
) -> Option<proc_macro2::TokenStream> {
    let ident = field.ident.as_ref()?;
    if is_flatten(field) || is_reference(field) {
        let ty = option_inner_type(&field.ty).unwrap_or(&field.ty);
        let ty_tokens = if projected {
            quote! { <#ty as #crate_path::queries::__private::Projectable>::Columns }
        } else {
            quote! { <#ty as #crate_path::queries::__private::HasCols>::Columns }
        };
        return Some(quote! { pub #ident: #ty_tokens, });
    }
    let ty = column_expr_ty(field, crate_path);
    let ty_tokens = if projected {
        quote! { #crate_path::queries::__private::ProjectedColumn<#ty> }
    } else {
        quote! { #crate_path::queries::__private::Column<#ty> }
    };
    Some(quote! { pub #ident: #ty_tokens, })
}

fn gen_cols_inits(
    fields: &[FieldMeta],
    crate_path: &proc_macro2::TokenStream,
    projected: bool,
    reference_owner: bool,
) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .filter(|field| !is_skip(field) && is_selectable(field))
        .filter_map(|field| gen_cols_init(field, crate_path, projected, reference_owner))
        .collect()
}

fn gen_cols_init(
    field: &FieldMeta,
    crate_path: &proc_macro2::TokenStream,
    projected: bool,
    reference_owner: bool,
) -> Option<proc_macro2::TokenStream> {
    let ident = field.ident.as_ref()?;
    if projected {
        return gen_projected_init(field, ident, crate_path);
    }
    if is_flatten(field) {
        let ty = &field.ty;
        if reference_owner {
            return Some(quote! {
                #ident: <#ty as #crate_path::queries::__private::HasCols>::cols_for_reference(reference),
            });
        }
        return Some(quote! {
            #ident: <#ty as #crate_path::queries::__private::HasCols>::cols_for_table(table),
        });
    }
    if is_reference(field) {
        let ty = option_inner_type(&field.ty).unwrap_or(&field.ty);
        let name = column_name(field);
        return Some(quote! {
            #ident: {
                let reference = #crate_path::queries::__private::reference(#name);
                <#ty as #crate_path::queries::__private::HasCols>::cols_for_reference(&reference)
            },
        });
    }
    let name = column_name(field);
    let ty = column_expr_ty(field, crate_path);
    if reference_owner {
        Some(quote! { #ident: reference.col::<#ty>(#name), })
    } else {
        Some(quote! { #ident: table.col::<#ty>(#name), })
    }
}

fn gen_projected_init(
    field: &FieldMeta,
    ident: &syn::Ident,
    crate_path: &proc_macro2::TokenStream,
) -> Option<proc_macro2::TokenStream> {
    if is_flatten(field) || is_reference(field) {
        let ty = option_inner_type(&field.ty).unwrap_or(&field.ty);
        return Some(quote! {
            #ident: <#ty as #crate_path::queries::__private::Projectable>::projected_columns(source.clone()),
        });
    }
    let name = column_name(field);
    let ty = column_expr_ty(field, crate_path);
    Some(quote! { #ident: source.col::<#ty>(#name), })
}

fn column_expr_ty(
    field: &FieldMeta,
    crate_path: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    if is_json(field) {
        quote! { #crate_path::types::Json }
    } else if let Some(inner) = array_inner_type(&field.ty) {
        quote! { #crate_path::types::Array<#inner> }
    } else {
        let ty = &field.ty;
        quote! { #ty }
    }
}

fn column_name(field: &FieldMeta) -> String {
    field
        .column
        .name
        .as_ref()
        .map(|lit| lit.value())
        .or_else(|| field.ident.as_ref().map(|ident| ident.to_string()))
        .unwrap_or_default()
}
