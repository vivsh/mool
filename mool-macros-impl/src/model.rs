use quote::quote;
use syn::{DeriveInput, GenericArgument, PathArguments, Type, TypePath};

use crate::schemable::{FieldMeta, ParsedStruct};

pub fn derive_model(
    input: proc_macro2::TokenStream,
    runtime_path: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let input = match syn::parse2::<DeriveInput>(input) {
        Ok(input) => input,
        Err(err) => return err.to_compile_error(),
    };
    derive_model_impl(&input, runtime_path)
}

fn derive_model_impl(
    input: &DeriveInput,
    runtime_path: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let parsed = match ParsedStruct::from_derive_input(input.clone()) {
        Ok(parsed) => parsed,
        Err(err) => return err.to_compile_error(),
    };

    let ident = &parsed.ident;
    let mut generics = parsed.generics.clone();
    let pk_fields = match primary_key_fields(&parsed) {
        Ok(fields) => fields,
        Err(err) => return err.to_compile_error(),
    };
    if pk_fields.is_empty() {
        return syn::Error::new_spanned(
            ident,
            "Model requires a primary key field or a field named `id`",
        )
        .to_compile_error();
    };

    let pk_idents = pk_fields
        .iter()
        .filter_map(|field| field.ident.as_ref())
        .collect::<Vec<_>>();
    let pk_types = pk_fields.iter().map(|field| &field.ty).collect::<Vec<_>>();
    let pk_columns = pk_fields
        .iter()
        .map(|field| column_name(field))
        .collect::<Vec<_>>();
    if let Some(field) = pk_fields
        .iter()
        .find(|field| field.column.updatable == Some(true))
    {
        return syn::Error::new_spanned(
            &field.ty,
            "model primary-key columns cannot be explicitly updateable",
        )
        .to_compile_error();
    }
    let pk_type = match pk_types.as_slice() {
        [ty] => quote! { #ty },
        _ => quote! { (#(#pk_types),*) },
    };
    let pk_value = match pk_idents.as_slice() {
        [ident] => quote! { self.#ident.clone() },
        _ => quote! { (#(self.#pk_idents.clone()),*) },
    };

    let record = crate::record::derive_record_impl(input, runtime_path.clone(), &pk_columns);
    let crate_path = crate::runtime_path(input, runtime_path);
    let wc = generics.where_clause.get_or_insert(syn::WhereClause {
        where_token: <syn::Token![where]>::default(),
        predicates: syn::punctuated::Punctuated::new(),
    });
    wc.predicates.push(syn::parse_quote! {
        #pk_type: ::core::clone::Clone + ::core::hash::Hash + ::core::cmp::Eq
    });
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let into_table = gen_into_table(&parsed, &crate_path);
    let sql_enum_schema = gen_sql_enum_schema(&parsed, &crate_path);
    let search_schema = gen_search_schema(&parsed, &crate_path);

    quote! {
        #record
        #into_table
        #sql_enum_schema

        impl #impl_generics #crate_path::Model for #ident #ty_generics #where_clause {
            type PrimaryKey = #pk_type;

            fn model_schema() -> #crate_path::ModelSchema<Self> {
                let schema = #crate_path::ModelSchema::new(
                    <Self as #crate_path::Record>::record_schema(),
                    &[#(#pk_columns),*],
                );
                #search_schema
                schema
            }

            fn primary_key(&self) -> Self::PrimaryKey {
                #pk_value
            }
        }
    }
}

fn primary_key_fields(parsed: &ParsedStruct) -> syn::Result<Vec<&FieldMeta>> {
    if let Some(primary_key) = &parsed.container.primary_key {
        let mut out = Vec::with_capacity(primary_key.columns.len());
        for column in primary_key.columns.iter() {
            let value = column.as_str();
            let Some(field) = parsed
                .fields
                .iter()
                .find(|field| column_name(field) == value)
            else {
                return Err(syn::Error::new(
                    proc_macro2::Span::call_site(),
                    format!("table primary_key references unknown column '{value}'"),
                ));
            };
            out.push(field);
        }
        return Ok(out);
    }
    let flagged = parsed
        .fields
        .iter()
        .filter(|field| field.column.primary_key)
        .collect::<Vec<_>>();
    if !flagged.is_empty() {
        return Ok(flagged);
    }
    Ok(parsed
        .fields
        .iter()
        .find(|field| field.ident.as_ref().is_some_and(|ident| ident == "id"))
        .into_iter()
        .collect())
}

fn gen_into_table(
    parsed: &ParsedStruct,
    crate_path: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let ident = &parsed.ident;
    let generics = parsed.generics.clone();
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let table_name = parsed
        .container
        .name
        .as_ref()
        .or(parsed.container.table.as_ref())
        .map(|lit| lit.value())
        .unwrap_or_else(|| crate::schemable::to_snake_case(&ident.to_string()));
    let schema_tokens = gen_schema_call(parsed.container.schema.as_ref().map(|lit| lit.value()));
    let columns = parsed
        .fields
        .iter()
        .filter(is_table_field)
        .filter_map(|field| gen_column(field, &table_name, crate_path));
    let indexes = parsed
        .fields
        .iter()
        .filter(is_table_field)
        .filter_map(|field| gen_index(field, crate_path));
    let constraints = parsed
        .fields
        .iter()
        .filter(is_table_field)
        .filter_map(gen_unique_constraint);
    let primary_key = gen_primary_key(parsed);
    let foreign_keys = parsed.container.foreign_keys.iter().map(gen_foreign_key);
    quote! {
        impl #impl_generics #crate_path::schema::IntoTable for #ident #ty_generics #where_clause {
            fn into_table(
                dialect: &#crate_path::gaman::core::Dialect,
            ) -> #crate_path::schema::Table {
                let mut table = #crate_path::schema::TableBuilder::new(#table_name);
                #schema_tokens
                #(#columns)*
                #primary_key
                #(#indexes)*
                #(#constraints)*
                #(#foreign_keys)*
                table.build()
            }
        }
    }
}

fn is_table_field(field: &&crate::schemable::FieldMeta) -> bool {
    !field.column.skip && !field.column.flatten && field.column.prefetch.is_none()
}

fn gen_schema_call(schema: Option<String>) -> Option<proc_macro2::TokenStream> {
    schema.map(|value| quote! { table = table.schema(#value); })
}

fn gen_column(
    field: &crate::schemable::FieldMeta,
    table_name: &str,
    crate_path: &proc_macro2::TokenStream,
) -> Option<proc_macro2::TokenStream> {
    let name = column_name(field);
    let ty = &field.ty;
    let inferred_array_element = array_inner_type(ty);
    #[cfg(feature = "vector")]
    let is_vector = crate::schemable::is_vector_type(ty);
    #[cfg(feature = "vector")]
    let vector_dimensions = field
        .column
        .vector
        .as_ref()
        .and_then(|spec| spec.dimensions.map(|dimensions| dimensions.0));
    let nullable_tokens = gen_nullable(
        field,
        field.column.serial || field.column.sql_type.is_some() || inferred_array_element.is_some(),
    );
    let pk = field
        .column
        .primary_key
        .then(|| quote! { let c = c.primary_key(); });
    let default = field.column.default.as_ref().map(|lit| {
        let value = lit.value();
        quote! { let c = c.default(#value); }
    });
    let check = field.column.check.as_ref().map(|lit| {
        let value = lit.value();
        quote! { let c = c.check(#value); }
    });
    let references = gen_reference(field);
    let body = quote! {
        #nullable_tokens
        #pk
        #default
        #check
        #references
        c
    };

    if field.column.serial {
        return Some(quote! {
            table = table.column(#name, "bigserial", |c| {
                #body
            });
        });
    }
    #[cfg(feature = "vector")]
    if is_vector {
        let dimensions = match vector_dimensions {
            Some(dimensions) => quote! { ::core::option::Option::Some(#dimensions) },
            None => quote! { ::core::option::Option::<usize>::None },
        };
        return Some(quote! {
            let sql_type = match (dialect, #dimensions) {
                (#crate_path::gaman::core::Dialect::Postgres, Some(dimensions)) => format!("vector({dimensions})"),
                (#crate_path::gaman::core::Dialect::Mariadb, Some(dimensions)) => format!("VECTOR({dimensions})"),
                (#crate_path::gaman::core::Dialect::Postgres, None) => "vector".to_string(),
                (#crate_path::gaman::core::Dialect::Mariadb, None) => "VECTOR".to_string(),
                (#crate_path::gaman::core::Dialect::Mysql, _) => "vector".to_string(),
                (#crate_path::gaman::core::Dialect::Sqlite, _) => "vector".to_string(),
            };
            table = table.column(#name, sql_type, |c| {
                #body
            });
        });
    }
    if field.column.sql_enum {
        let enum_ty = option_inner_type(ty).unwrap_or(ty);
        let check_name = format!("ck_{table_name}_{name}_sql_enum");
        return Some(quote! {
            table = table.column(
                #name,
                <#enum_ty as #crate_path::SqlEnum>::sql_column_type(*dialect),
                |c| {
                    #body
                },
            );
            if let Some(check) = <#enum_ty as #crate_path::SqlEnum>::sql_check_expr(#name, *dialect) {
                table = table.check(#check_name, check);
            }
        });
    }
    if let Some(sql_type) = field.column.sql_type.as_ref().map(|lit| lit.value()) {
        return Some(quote! {
            table = table.column(#name, #sql_type, |c| {
                #body
            });
        });
    }
    if let Some(element_type) = inferred_array_element {
        return Some(quote! {
            let sql_type = #crate_path::schema::__private::native_array_sql_type::<#element_type>();
            table = table.column(#name, sql_type, |c| {
                #body
            });
        });
    }
    Some(quote! {
        let column_desc = <#ty as #crate_path::schema::ColumnType>::column_desc(dialect);
        table = table.column(#name, column_desc.sql_type, |c| {
            let c = if column_desc.nullable {
                c.nullable()
            } else {
                c.not_null()
            };
            #body
        });
    })
}

#[cfg(any(feature = "text-search", feature = "fuzzy-search", feature = "vector"))]
fn gen_search_schema(
    parsed: &ParsedStruct,
    crate_path: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let metadata = gen_search_metadata(parsed, crate_path);
    quote! { let schema = schema.search(#metadata); }
}

#[cfg(not(any(feature = "text-search", feature = "fuzzy-search", feature = "vector")))]
fn gen_search_schema(
    _parsed: &ParsedStruct,
    _crate_path: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    proc_macro2::TokenStream::new()
}

#[cfg(any(feature = "text-search", feature = "fuzzy-search", feature = "vector"))]
fn gen_search_metadata(
    parsed: &ParsedStruct,
    crate_path: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let mut registrations = Vec::new();
    #[cfg(feature = "text-search")]
    registrations.extend(gen_full_text_metadata(parsed, crate_path));
    #[cfg(feature = "fuzzy-search")]
    registrations.extend(gen_fuzzy_metadata(parsed, crate_path));
    #[cfg(feature = "vector")]
    registrations.extend(gen_vector_metadata(parsed, crate_path));
    quote! {{
        let mut search = #crate_path::search::SearchTableMetadata::new();
        #(#registrations)*
        search
    }}
}

#[cfg(feature = "text-search")]
fn gen_full_text_metadata(
    parsed: &ParsedStruct,
    crate_path: &proc_macro2::TokenStream,
) -> Vec<proc_macro2::TokenStream> {
    let mut registrations = Vec::new();
    for definition in &parsed.container.full_text {
        let name = &definition.name;
        let columns = definition.columns.iter().collect::<Vec<_>>();
        let index_name = definition.index_name.as_ref().map(|value| {
            quote! { definition = definition.index_name(#value); }
        });
        let config = definition
            .postgres
            .as_ref()
            .and_then(|value| value.config.as_ref())
            .map(|value| quote! { definition = definition.postgres_config(#value); });
        registrations.push(quote! {
            let mut definition = #crate_path::search::FullTextDefinition::new(
                #name,
                [#(#columns),*],
            );
            #index_name
            #config
            search = search.full_text(definition);
        });
    }
    registrations
}

#[cfg(feature = "fuzzy-search")]
fn gen_fuzzy_metadata(
    parsed: &ParsedStruct,
    crate_path: &proc_macro2::TokenStream,
) -> Vec<proc_macro2::TokenStream> {
    let mut registrations = Vec::new();
    for field in parsed
        .fields
        .iter()
        .filter(|field| field.column.fuzzy.is_some())
    {
        let column = column_name(field);
        let Some(spec) = field.column.fuzzy.as_ref() else {
            continue;
        };
        let index = match spec.index.as_deref().unwrap_or("none") {
            "gist" => quote! { #crate_path::search::FuzzyIndex::GiSt },
            "gin" => quote! { #crate_path::search::FuzzyIndex::Gin },
            _ => quote! { #crate_path::search::FuzzyIndex::None },
        };
        let index_name = spec.index_name.as_ref().map(|value| {
            quote! { definition = definition.index_name(#value); }
        });
        registrations.push(quote! {
            let mut definition = #crate_path::search::FuzzyDefinition::new(#column)
                .index(#index);
            #index_name
            search = search.fuzzy(definition);
        });
    }
    registrations
}

#[cfg(feature = "vector")]
fn gen_vector_metadata(
    parsed: &ParsedStruct,
    crate_path: &proc_macro2::TokenStream,
) -> Vec<proc_macro2::TokenStream> {
    parsed
        .fields
        .iter()
        .filter_map(|field| gen_vector_registration(field, crate_path))
        .collect()
}

#[cfg(feature = "vector")]
fn gen_vector_registration(
    field: &FieldMeta,
    crate_path: &proc_macro2::TokenStream,
) -> Option<proc_macro2::TokenStream> {
    if !crate::schemable::is_vector_type(&field.ty) {
        return None;
    }
    let column = column_name(field);
    let spec = field.column.vector.as_ref();
    let dimensions = spec
        .and_then(|value| value.dimensions)
        .map(|dimensions| dimensions.0)
        .map(|value| {
            quote! { definition = definition.dimensions(#value); }
        });
    let metric = vector_metric_tokens(spec.and_then(|value| value.metric.as_deref()), crate_path);
    let index = vector_index_tokens(spec.and_then(|value| value.index.as_deref()), crate_path);
    let index_name = spec
        .and_then(|value| value.index_name.as_ref())
        .map(|value| {
            quote! { definition = definition.index_name(#value); }
        });
    let postgres = spec.and_then(postgres_vector_tokens);
    let mariadb = spec
        .and_then(|value| value.mariadb.as_ref())
        .and_then(|options| options.m)
        .map(|value| quote! { definition = definition.mariadb_m(#value); });
    Some(quote! {
        let mut definition = #crate_path::search::VectorDefinition::new(#column)
            .metric(#metric)
            .index(#index);
        #dimensions
        #index_name
        #postgres
        #mariadb
        search = search.vector(definition);
    })
}

#[cfg(feature = "vector")]
fn vector_metric_tokens(
    metric: Option<&str>,
    crate_path: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    match metric.unwrap_or("cosine") {
        "l2" => quote! { #crate_path::search::VectorMetric::L2 },
        _ => quote! { #crate_path::search::VectorMetric::Cosine },
    }
}

#[cfg(feature = "vector")]
fn vector_index_tokens(
    index: Option<&str>,
    crate_path: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    match index.unwrap_or("none") {
        "hnsw" => quote! { #crate_path::search::VectorIndex::Hnsw },
        "ivfflat" => quote! { #crate_path::search::VectorIndex::IvfFlat },
        _ => quote! { #crate_path::search::VectorIndex::None },
    }
}

#[cfg(feature = "vector")]
fn postgres_vector_tokens(spec: &crate::schemable::VectorSpec) -> Option<proc_macro2::TokenStream> {
    spec.postgres.as_ref().map(|options| {
        let m = quote_option(options.m);
        let ef = quote_option(options.ef_construction);
        let lists = options
            .lists
            .map(|value| quote! { definition = definition.postgres_lists(#value); });
        quote! {
            definition = definition.postgres_hnsw(#m, #ef);
            #lists
        }
    })
}

#[cfg(feature = "vector")]
fn quote_option<T>(value: Option<T>) -> proc_macro2::TokenStream
where
    T: quote::ToTokens,
{
    match value {
        Some(value) => quote! { ::core::option::Option::Some(#value) },
        None => quote! { ::core::option::Option::None },
    }
}

fn gen_nullable(field: &FieldMeta, explicit_type: bool) -> Option<proc_macro2::TokenStream> {
    match field.column.nullable {
        Some(true) => Some(quote! { let c = c.nullable(); }),
        Some(false) => Some(quote! { let c = c.not_null(); }),
        None if explicit_type && is_option_type(&field.ty) => {
            Some(quote! { let c = c.nullable(); })
        }
        None if explicit_type => Some(quote! { let c = c.not_null(); }),
        None => None,
    }
}

fn gen_reference(field: &crate::schemable::FieldMeta) -> Option<proc_macro2::TokenStream> {
    let reference = field.column.reference.as_ref()?;
    let references = reference.target.as_ref()?;
    let (table, column) = references.rsplit_once('.')?;
    let name = reference.name.clone();
    Some(match name {
        Some(name) => quote! {
            let c = c.references_named(#name, #table, #column);
        },
        None => quote! {
            let c = c.references(#table, #column);
        },
    })
}

fn gen_index(
    field: &crate::schemable::FieldMeta,
    crate_path: &proc_macro2::TokenStream,
) -> Option<proc_macro2::TokenStream> {
    if !field.column.index && field.column.index_name.is_none() {
        return None;
    }
    let name = column_name(field);
    Some(
        match field.column.index_name.as_ref().map(|lit| lit.value()) {
            Some(index_name) => quote! {
                table = table.index(
                    #crate_path::schema::Index::columns([#name]).named(#index_name),
                );
            },
            None => quote! {
                table = table.index(#crate_path::schema::Index::columns([#name]));
            },
        },
    )
}

fn gen_unique_constraint(field: &crate::schemable::FieldMeta) -> Option<proc_macro2::TokenStream> {
    if !field.column.unique && field.column.unique_name.is_none() {
        return None;
    }
    let name = column_name(field);
    Some(
        match field.column.unique_name.as_ref().map(|lit| lit.value()) {
            Some(unique_name) => quote! { table = table.unique(#unique_name, &[#name]); },
            None => quote! { table = table.unique_columns(&[#name]); },
        },
    )
}

fn gen_primary_key(parsed: &ParsedStruct) -> Option<proc_macro2::TokenStream> {
    let primary_key = parsed.container.primary_key.as_ref()?;
    let columns = primary_key.columns.iter().collect::<Vec<_>>();
    Some(match primary_key.name.as_ref() {
        Some(name) => quote! { table = table.primary_key(#name, &[#(#columns),*]); },
        None => quote! { table = table.primary_key_columns(&[#(#columns),*]); },
    })
}

fn gen_foreign_key(foreign_key: &crate::schemable::ForeignKeySpec) -> proc_macro2::TokenStream {
    let columns = foreign_key.columns.iter().collect::<Vec<_>>();
    let target_table = &foreign_key.references.table;
    let target_columns = foreign_key.references.columns.iter().collect::<Vec<_>>();
    match foreign_key.name.as_ref() {
        Some(name) => {
            quote! { table = table.foreign_key_named_columns(#name, &[#(#columns),*], #target_table, &[#(#target_columns),*]); }
        }
        None => {
            quote! { table = table.foreign_key_columns(&[#(#columns),*], #target_table, &[#(#target_columns),*]); }
        }
    }
}

fn gen_sql_enum_schema(
    parsed: &ParsedStruct,
    crate_path: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let ident = &parsed.ident;
    let generics = parsed.generics.clone();
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let enum_types = sql_enum_types(parsed);
    let registrations = enum_types.iter().map(|ty| {
        quote! {
            #crate_path::enums::SqlEnumRegistration::new(
                #crate_path::enums::__private::register_enum::<#ty>,
            )
        }
    });
    quote! {
        impl #impl_generics #crate_path::enums::SqlEnumSchema for #ident #ty_generics #where_clause {
            const SQL_ENUMS: &'static [#crate_path::enums::SqlEnumRegistration] = &[
                #(#registrations),*
            ];
        }
    }
}

fn sql_enum_types(parsed: &ParsedStruct) -> Vec<&Type> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for field in &parsed.fields {
        if !field.column.sql_enum {
            continue;
        }
        let ty = option_inner_type(&field.ty).unwrap_or(&field.ty);
        let key = quote::quote!(#ty).to_string();
        if seen.insert(key) {
            out.push(ty);
        }
    }
    out
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

fn is_option_type(ty: &syn::Type) -> bool {
    option_inner_type(ty).is_some()
}

fn array_inner_type(ty: &Type) -> Option<&Type> {
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
    if is_u8_type(inner) {
        return None;
    }
    Some(inner)
}

fn option_inner_type(ty: &Type) -> Option<&Type> {
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
