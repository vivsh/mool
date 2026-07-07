use quote::quote;
use std::collections::HashSet;
use syn::{DeriveInput, GenericArgument, PathArguments, Type, TypePath};

use crate::schemable::{FieldMeta, ParsedStruct, to_snake_case};
use crate::typed_handles::gen_typed_handles;

/// Derives the Record trait for deserializing database rows into structs.
pub fn derive_record(
    input: proc_macro2::TokenStream,
    runtime_path: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let input = match syn::parse2::<DeriveInput>(input) {
        Ok(input) => input,
        Err(err) => return err.to_compile_error(),
    };
    derive_record_impl(&input, runtime_path)
}

/// Internal implementation of Record derive macro.
pub(crate) fn derive_record_impl(
    input: &DeriveInput,
    runtime_path: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let parsed = match ParsedStruct::from_derive_input(input.clone()) {
        Ok(p) => p,
        Err(e) => return e.to_compile_error(),
    };

    let ident = &parsed.ident;
    let mut generics = parsed.generics.clone();

    let crate_path = crate::runtime_path(input, runtime_path);

    gen_where_clause(&mut generics, &parsed.fields, &crate_path);

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let table_name = gen_table_name(&parsed, &crate_path);
    let table_schema = gen_table_schema(&parsed);
    let scan_root = gen_scan_root(&parsed.fields);
    let references = gen_references(&parsed.fields, &crate_path);
    let field_inits = gen_field_initializers(&parsed.fields, &crate_path);
    let field_inits_unordered = gen_field_initializers_unordered(&parsed.fields, &crate_path);
    let column_names = gen_record_column_names(&parsed.fields, &crate_path);
    let bind_names = gen_record_bind_column_names(&parsed.fields, &crate_path);
    let bind_stmts = gen_bind_statements(&parsed.fields, &crate_path);
    let bind_selected_arms = gen_bind_selected_arms(&parsed.fields, &crate_path);
    let typed_handles = gen_typed_handles(&parsed, &crate_path);

    quote! {
        impl #impl_generics #crate_path::Record for #ident #ty_generics #where_clause {
            fn record_schema() -> #crate_path::RecordSchema<Self> {
                let references = {
                    let mut refs = ::std::vec::Vec::new();
                    #(#references)*
                    refs
                };
                let columns = {
                    let mut cols = ::std::vec::Vec::new();
                    #(#column_names)*
                    cols
                };
                let bind_columns = {
                    let mut cols = ::std::vec::Vec::new();
                    #(#bind_names)*
                    cols
                };
                #crate_path::RecordSchema::new(#table_name)
                    .schema(#table_schema)
                    .root(#scan_root)
                    .references(references)
                    .columns(columns)
                    .bind_columns(bind_columns)
            }

            fn record_bind_values(
                &self,
                args: &mut #crate_path::Arguments<'static>,
            ) -> Result<(), ::sqlx::Error> {
                #(#bind_stmts)*
                Ok(())
            }

            fn record_bind_selected(
                &self,
                columns: &[&str],
                args: &mut #crate_path::Arguments<'static>,
            ) -> Result<(), ::sqlx::Error> {
                for column in columns {
                    match *column {
                        #(#bind_selected_arms)*
                        other => {
                            return Err(::sqlx::Error::ColumnNotFound(other.to_string()));
                        }
                    }
                }
                Ok(())
            }

            fn record_scan_ordered(
                row: &#crate_path::Row,
                start_idx: &mut usize,
            ) -> Result<Self, ::sqlx::Error> {
                use ::sqlx::Row as _;
                use ::sqlx::ValueRef as _;
                Ok(Self {
                    #(#field_inits)*
                })
            }

            fn record_scan_unordered(
                row: &#crate_path::Row,
            ) -> Result<Self, ::sqlx::Error> {
                use ::sqlx::Row as _;
                Ok(Self {
                    #(#field_inits_unordered)*
                })
            }
        }

        impl #impl_generics ::sqlx::FromRow<'_, #crate_path::Row> for #ident #ty_generics #where_clause {
            fn from_row(row: &#crate_path::Row) -> Result<Self, ::sqlx::Error> {
                <Self as #crate_path::Record>::record_scan(row)
            }
        }

        #typed_handles
    }
}

/// Generate where clause predicates for Record trait bounds.
fn gen_where_clause(
    generics: &mut syn::Generics,
    fields: &[FieldMeta],
    crate_path: &proc_macro2::TokenStream,
) {
    let mut seen = HashSet::new();
    let wc = generics.where_clause.get_or_insert(syn::WhereClause {
        where_token: <syn::Token![where]>::default(),
        predicates: syn::punctuated::Punctuated::new(),
    });

    for field in fields {
        if is_skip(field) {
            continue;
        }

        let ty = &field.ty;
        let ty_str = quote::quote!(#ty).to_string();

        if (is_flatten(field) || is_reference(field)) && is_selectable(field) {
            let bound_ty = option_inner_type(ty).unwrap_or(ty);
            let bound_ty_str = quote::quote!(#bound_ty).to_string();
            if seen.insert(bound_ty_str) {
                wc.predicates.push(syn::parse_quote! {
                    #bound_ty: #crate_path::Record
                });
            }
        } else if is_json(field) && is_selectable(field) && seen.insert(ty_str.clone()) {
            wc.predicates.push(syn::parse_quote! {
                #ty: ::serde::de::DeserializeOwned
            });
        }

        if !is_bindable(field) {
            continue;
        }

        if is_flatten(field) {
            if seen.insert(ty_str.clone()) {
                wc.predicates.push(syn::parse_quote! {
                    #ty: #crate_path::Record
                });
            }
        } else if is_json(field) {
            if seen.insert(format!("{ty_str}:serialize")) {
                wc.predicates.push(syn::parse_quote! {
                    #ty: ::serde::Serialize
                });
            }
        } else {
            wc.predicates.push(syn::parse_quote! {
                #ty: ::core::clone::Clone
                    + for<'q> ::sqlx::Encode<'q, #crate_path::Database>
                    + ::sqlx::Type<#crate_path::Database>
                    + ::core::marker::Send
            });
        }
    }
}

/// Generate field initializers for struct construction.
fn gen_field_initializers(
    fields: &[FieldMeta],
    crate_path: &proc_macro2::TokenStream,
) -> Vec<proc_macro2::TokenStream> {
    let mut inits = Vec::with_capacity(fields.len());

    for field in fields {
        let Some(ident) = &field.ident else {
            continue;
        };

        let init = if is_skip(field) || !is_selectable(field) {
            gen_default_init(ident)
        } else if is_reference(field) && is_option(&field.ty) {
            gen_optional_reference_init(ident, &field.ty, crate_path)
        } else if is_flatten(field) || is_reference(field) {
            gen_flatten_init(
                ident,
                option_inner_type(&field.ty).unwrap_or(&field.ty),
                crate_path,
            )
        } else if is_json(field) {
            gen_json_init(ident, crate_path)
        } else {
            gen_scalar_init(ident)
        };

        inits.push(init);
    }

    inits
}

fn gen_table_name(
    parsed: &ParsedStruct,
    crate_path: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    if let Some(name) = parsed
        .container
        .name
        .as_ref()
        .or(parsed.container.table.as_ref())
    {
        let value = name.value();
        return quote! { #value };
    }

    if let Some(field) = parsed
        .fields
        .iter()
        .find(|field| is_flatten(field) || is_reference(field))
    {
        let ty = option_inner_type(&field.ty).unwrap_or(&field.ty);
        return quote! { <#ty as #crate_path::Record>::record_table_name() };
    }

    let value = to_snake_case(&parsed.ident.to_string());
    quote! { #value }
}

fn gen_table_schema(parsed: &ParsedStruct) -> proc_macro2::TokenStream {
    if let Some(schema) = parsed.container.schema.as_ref() {
        let value = schema.value();
        quote! { Some(#value) }
    } else {
        quote! { None }
    }
}

fn gen_scan_root(fields: &[FieldMeta]) -> proc_macro2::TokenStream {
    if let Some(field) = fields
        .iter()
        .find(|field| is_flatten(field) || is_reference(field))
        && let Some(ident) = &field.ident
    {
        let value = ident.to_string();
        return quote! { Some(#value) };
    }

    quote! { None }
}

fn gen_references(
    fields: &[FieldMeta],
    crate_path: &proc_macro2::TokenStream,
) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .filter(|field| is_reference(field))
        .filter_map(|field| {
            let ident = field.ident.as_ref()?;
            let logical_name = ident.to_string();
            let ty = option_inner_type(&field.ty).unwrap_or(&field.ty);
            if let Some(backref) = field.column.backref.as_ref() {
                let marker = &backref.path;
                return Some(quote! {
                    let backref = <#marker as #crate_path::Backref>::meta();
                    refs.push(#crate_path::ReferenceMeta {
                        logical_name: #logical_name,
                        table_name: backref.table_name,
                        table_schema: backref.table_schema,
                        columns: backref.columns,
                        join_type: backref.join_type,
                    });
                });
            }

            let reference = field.column.reference.as_ref()?;
            if !reference.is_join_reference() {
                return None;
            }
            if let Some(marker) = reference.relation.as_ref() {
                return Some(quote! {
                    let relation = <#marker as #crate_path::Backref>::meta();
                    refs.push(#crate_path::ReferenceMeta {
                        logical_name: #logical_name,
                        table_name: relation.table_name,
                        table_schema: relation.table_schema,
                        columns: relation.columns,
                        join_type: relation.join_type,
                    });
                });
            }
            let join_type = reference_join_type(reference, &field.ty, crate_path);
            let columns = reference_columns(reference, crate_path);

            Some(quote! {
                refs.push(#crate_path::ReferenceMeta {
                    logical_name: #logical_name,
                    table_name: <#ty as #crate_path::Record>::record_table_name(),
                    table_schema: <#ty as #crate_path::Record>::record_table_schema(),
                    columns: &[#(#columns),*],
                    join_type: #join_type,
                });
            })
        })
        .collect()
}

fn reference_join_type(
    reference: &crate::schemable::ReferenceSpec,
    ty: &Type,
    crate_path: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    match reference.join.as_deref() {
        Some("left") => quote! { #crate_path::JoinType::Left },
        Some("inner") => quote! { #crate_path::JoinType::Inner },
        _ if is_option(ty) => quote! { #crate_path::JoinType::Left },
        _ => quote! { #crate_path::JoinType::Inner },
    }
}

fn reference_columns(
    reference: &crate::schemable::ReferenceSpec,
    crate_path: &proc_macro2::TokenStream,
) -> Vec<proc_macro2::TokenStream> {
    if !reference.on.is_empty() {
        return reference
            .on
            .iter()
            .map(|column| {
                let from = column.from.as_str();
                let to = column.to.as_str();
                quote! { #crate_path::JoinColumn { from: #from, to: #to } }
            })
            .collect();
    }
    let from = reference.from.as_deref().unwrap_or("");
    let to = reference.to.as_deref().unwrap_or("id");
    vec![quote! { #crate_path::JoinColumn { from: #from, to: #to } }]
}

/// Generate field initializers for unordered (name-based) struct construction.
fn gen_field_initializers_unordered(
    fields: &[FieldMeta],
    crate_path: &proc_macro2::TokenStream,
) -> Vec<proc_macro2::TokenStream> {
    let mut inits = Vec::with_capacity(fields.len());

    for field in fields {
        let Some(ident) = &field.ident else {
            continue;
        };

        let init = if is_skip(field) || !is_selectable(field) {
            gen_default_init(ident)
        } else if is_reference(field) {
            // Reference fields cannot be scanned unordered - they need prefixed column names
            gen_reference_unordered_error(ident, &field.ty)
        } else if is_flatten(field) {
            gen_flatten_init_unordered(ident, &field.ty, crate_path)
        } else if is_json(field) {
            gen_json_init_unordered(ident, field, crate_path)
        } else {
            gen_scalar_init_unordered(ident, field)
        };

        inits.push(init);
    }

    inits
}

/// Generate default initialization for non-selectable field.
fn gen_default_init(ident: &syn::Ident) -> proc_macro2::TokenStream {
    quote! {
        #ident: ::core::default::Default::default(),
    }
}

/// Generate compile error for reference field in unordered scan.
fn gen_reference_unordered_error(ident: &syn::Ident, ty: &Type) -> proc_macro2::TokenStream {
    let error_msg = format!(
        "Cannot use record_scan_unordered with reference field '{}' of type '{}'. \
        Reference fields require ordered scanning (record_scan_ordered) because they use \
        prefixed column names. Use record_scan_ordered or scan_row instead.",
        ident,
        quote::quote!(#ty)
    );
    quote! {
        #ident: {
            let unsupported: Result<#ty, ::sqlx::Error> =
                Err(::sqlx::Error::ColumnNotFound(#error_msg.to_string()));
            unsupported?
        },
    }
}

/// Generate initialization for flattened field.
fn gen_flatten_init(
    ident: &syn::Ident,
    ty: &Type,
    crate_path: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    quote! {
        #ident: <#ty as #crate_path::Record>::record_scan_ordered(row, start_idx)?,
    }
}

fn gen_optional_reference_init(
    ident: &syn::Ident,
    ty: &Type,
    crate_path: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let inner_ty = option_inner_type(ty).unwrap_or(ty);
    quote! {
        #ident: {
            let nested_cols = <#inner_ty as #crate_path::Record>::record_column_names();
            let start = *start_idx;
            let mut all_null = true;
            for offset in 0..nested_cols.len() {
                let raw = row.try_get_raw(start + offset)?;
                if !raw.is_null() {
                    all_null = false;
                    break;
                }
            }
            if all_null {
                *start_idx += nested_cols.len();
                None
            } else {
                Some(<#inner_ty as #crate_path::Record>::record_scan_ordered(row, start_idx)?)
            }
        },
    }
}

/// Generate initialization for JSON-deserialized field.
fn gen_json_init(
    ident: &syn::Ident,
    _crate_path: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    quote! {
        #ident: {
            let json_val: ::serde_json::Value = row.try_get(*start_idx)?;
            *start_idx += 1;
            ::serde_json::from_value(json_val)
                .map_err(|e| ::sqlx::Error::Decode(Box::new(e)))?
        },
    }
}

/// Generate initialization for scalar field.
fn gen_scalar_init(ident: &syn::Ident) -> proc_macro2::TokenStream {
    quote! {
        #ident: {
            let val = row.try_get(*start_idx)?;
            *start_idx += 1;
            val
        },
    }
}

/// Check if field should be skipped (column only).
pub(super) fn is_skip(field: &FieldMeta) -> bool {
    field.column.skip || field.column.prefetch.is_some()
}

/// Check if field should be flattened (column only).
pub(super) fn is_flatten(field: &FieldMeta) -> bool {
    field.column.flatten
}

/// Check if field is a reference (column only).
pub(super) fn is_reference(field: &FieldMeta) -> bool {
    field
        .column
        .reference
        .as_ref()
        .is_some_and(|reference| reference.is_join_reference())
        || field.column.backref.is_some()
}

fn is_bindable(field: &FieldMeta) -> bool {
    !is_skip(field)
        && !is_reference(field)
        && !field.column.read_only
        && !field.column.skip_bind
        && field.column.insertable.unwrap_or(true)
        && field.column.updatable.unwrap_or(true)
}

fn is_option(ty: &Type) -> bool {
    option_inner_type(ty).is_some()
}

pub(super) fn option_inner_type(ty: &Type) -> Option<&Type> {
    let Type::Path(path) = ty else {
        return None;
    };
    if !is_canonical_option(path) {
        return None;
    }
    let segment = path.path.segments.last()?;
    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return None;
    };
    let Some(GenericArgument::Type(inner)) = args.args.first() else {
        return None;
    };
    if is_u8_type(inner) {
        return None;
    }
    Some(inner)
}

fn is_u8_type(ty: &Type) -> bool {
    let Type::Path(path) = ty else {
        return false;
    };
    if path.qself.is_some() {
        return false;
    }
    let mut segments = path
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string());
    matches!(
        (
            segments.next(),
            segments.next(),
            segments.next(),
            segments.next()
        ),
        (Some(first), None, None, None) if first == "u8"
    )
}

pub(super) fn array_inner_type(ty: &Type) -> Option<&Type> {
    let candidate = option_inner_type(ty).unwrap_or(ty);
    let Type::Path(path) = candidate else {
        return None;
    };
    if !is_canonical_vec(path) {
        return None;
    }
    let segment = path.path.segments.last()?;
    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return None;
    };
    let Some(GenericArgument::Type(inner)) = args.args.first() else {
        return None;
    };
    Some(inner)
}

fn is_canonical_option(path: &TypePath) -> bool {
    if path.qself.is_some() {
        return false;
    }
    let mut segments = path
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string());
    match (
        segments.next(),
        segments.next(),
        segments.next(),
        segments.next(),
    ) {
        (Some(first), None, None, None) => first == "Option",
        (Some(first), Some(second), Some(third), None) => {
            (first == "std" || first == "core") && second == "option" && third == "Option"
        }
        _ => false,
    }
}

fn is_canonical_vec(path: &TypePath) -> bool {
    if path.qself.is_some() {
        return false;
    }
    let mut segments = path
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string());
    match (
        segments.next(),
        segments.next(),
        segments.next(),
        segments.next(),
    ) {
        (Some(first), None, None, None) => first == "Vec",
        (Some(first), Some(second), Some(third), None) => {
            (first == "std" || first == "alloc") && second == "vec" && third == "Vec"
        }
        _ => false,
    }
}

/// Check if field should be JSON-serialized (column only).
pub(super) fn is_json(field: &FieldMeta) -> bool {
    field.column.json
        || field
            .column
            .sql_type
            .as_ref()
            .map(|ty| matches!(ty.value().to_ascii_lowercase().as_str(), "json" | "jsonb"))
            .unwrap_or(false)
}

/// Check if field is selectable (column attr only, None means true).
pub(super) fn is_selectable(field: &FieldMeta) -> bool {
    field.column.selectable.unwrap_or(true)
}

/// Generate the record_column_names implementation.
fn gen_record_column_names(
    fields: &[FieldMeta],
    crate_path: &proc_macro2::TokenStream,
) -> Vec<proc_macro2::TokenStream> {
    let mut stmts = Vec::new();

    for field in fields {
        if is_skip(field) || !is_selectable(field) {
            continue;
        }

        if is_reference(field) {
            let ty = option_inner_type(&field.ty).unwrap_or(&field.ty);
            let field_name = field
                .column
                .name
                .as_ref()
                .map(|lit| lit.value())
                .or_else(|| field.ident.as_ref().map(|i| i.to_string()))
                .unwrap_or_default();

            stmts.push(quote! {
                {
                    let nested_cols = <#ty as #crate_path::Record>::record_column_names();
                    for col in nested_cols {
                        cols.push(format!("{}.{}", #field_name, col));
                    }
                }
            });
        } else if is_flatten(field) {
            let ty = &field.ty;
            stmts.push(quote! {
                cols.extend(<#ty as #crate_path::Record>::record_column_names());
            });
        } else {
            let col_name = field
                .column
                .name
                .as_ref()
                .map(|lit| lit.value())
                .or_else(|| field.ident.as_ref().map(|i| i.to_string()))
                .unwrap_or_default();
            stmts.push(quote! {
                cols.push(#col_name.to_string());
            });
        }
    }

    stmts
}

fn gen_record_bind_column_names(
    fields: &[FieldMeta],
    crate_path: &proc_macro2::TokenStream,
) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .filter(|field| is_bindable(field))
        .filter_map(|field| {
            if is_flatten(field) {
                let ty = &field.ty;
                return Some(quote! {
                    cols.extend(<#ty as #crate_path::Record>::record_bind_column_names());
                });
            }
            let col_name = field
                .column
                .name
                .as_ref()
                .map(|lit| lit.value())
                .or_else(|| field.ident.as_ref().map(|ident| ident.to_string()))?;
            Some(quote! {
                cols.push(#col_name.to_string());
            })
        })
        .collect()
}

fn gen_bind_statements(
    fields: &[FieldMeta],
    crate_path: &proc_macro2::TokenStream,
) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .filter(|field| is_bindable(field))
        .filter_map(|field| {
            let ident = field.ident.as_ref()?;
            if is_flatten(field) {
                let ty = &field.ty;
                return Some(quote! {
                    <#ty as #crate_path::Record>::record_bind_values(&self.#ident, args)?;
                });
            }
            if is_json(field) {
                return Some(quote! {
                    {
                        let value = ::serde_json::to_value(&self.#ident)
                            .map_err(|err| ::sqlx::Error::Decode(Box::new(err)))?;
                        ::sqlx::Arguments::add(args, value)
                            .map_err(::sqlx::Error::Decode)?;
                    }
                });
            }
            Some(quote! {
                {
                    ::sqlx::Arguments::add(args, self.#ident.clone())
                        .map_err(::sqlx::Error::Decode)?;
                }
            })
        })
        .collect()
}

fn gen_bind_selected_arms(
    fields: &[FieldMeta],
    crate_path: &proc_macro2::TokenStream,
) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .filter(|field| is_bindable(field))
        .filter_map(|field| {
            let ident = field.ident.as_ref()?;
            if is_flatten(field) {
                let ty = &field.ty;
                return Some(quote! {
                    nested if <#ty as #crate_path::Record>::record_bind_column_names()
                        .iter()
                        .any(|name| name == nested) => {
                        <#ty as #crate_path::Record>::record_bind_selected(
                            &self.#ident,
                            &[nested],
                            args,
                        )?;
                    }
                });
            }
            let col_name = field
                .column
                .name
                .as_ref()
                .map(|lit| lit.value())
                .or_else(|| field.ident.as_ref().map(|ident| ident.to_string()))?;
            if is_json(field) {
                return Some(quote! {
                    #col_name => {
                        let value = ::serde_json::to_value(&self.#ident)
                            .map_err(|err| ::sqlx::Error::Decode(Box::new(err)))?;
                        ::sqlx::Arguments::add(args, value)
                            .map_err(::sqlx::Error::Decode)?;
                    }
                });
            }
            Some(quote! {
                #col_name => {
                    ::sqlx::Arguments::add(args, self.#ident.clone())
                        .map_err(::sqlx::Error::Decode)?;
                }
            })
        })
        .collect()
}

/// Generate initialization for flattened field (unordered).
fn gen_flatten_init_unordered(
    ident: &syn::Ident,
    ty: &Type,
    crate_path: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    quote! {
        #ident: <#ty as #crate_path::Record>::record_scan_unordered(row)?,
    }
}

/// Generate initialization for JSON-deserialized field (unordered).
fn gen_json_init_unordered(
    ident: &syn::Ident,
    field: &FieldMeta,
    _crate_path: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let col_name = field
        .column
        .name
        .as_ref()
        .map(|lit| lit.value())
        .unwrap_or_else(|| ident.to_string());

    quote! {
        #ident: {
            let json_val: ::serde_json::Value = row.try_get(#col_name)?;
            ::serde_json::from_value(json_val)
                .map_err(|e| ::sqlx::Error::Decode(Box::new(e)))?
        },
    }
}

/// Generate initialization for scalar field (unordered).
fn gen_scalar_init_unordered(ident: &syn::Ident, field: &FieldMeta) -> proc_macro2::TokenStream {
    let col_name = field
        .column
        .name
        .as_ref()
        .map(|lit| lit.value())
        .unwrap_or_else(|| ident.to_string());

    quote! {
        #ident: row.try_get(#col_name)?,
    }
}
