use darling::FromMeta;
use quote::quote;
use syn::{
    DeriveInput, Error, GenericArgument, Ident, Path, PathArguments, Type, spanned::Spanned,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum FilterOp {
    Eq,
    Ne,
    Lt,
    Lte,
    Gt,
    Gte,
    Like,
    ILike,
    In,
}

impl FilterOp {
    fn method(self) -> Ident {
        Ident::new(
            match self {
                FilterOp::Eq => "eq",
                FilterOp::Ne => "ne",
                FilterOp::Lt => "lt",
                FilterOp::Lte => "lte",
                FilterOp::Gt => "gt",
                FilterOp::Gte => "gte",
                FilterOp::Like => "like",
                FilterOp::ILike => "ilike",
                FilterOp::In => "in",
            },
            proc_macro2::Span::call_site(),
        )
    }
}

struct FilterAttr {
    op: FilterOp,
    column: Option<String>,
    span: proc_macro2::Span,
}

#[derive(Default, FromMeta)]
struct FilterContainer {
    model: Option<Path>,
}

#[derive(Default, FromMeta)]
struct FilterMeta {
    #[darling(default)]
    op: Option<FilterOp>,
    #[darling(default)]
    column: Option<String>,
}

impl FromMeta for FilterOp {
    fn from_string(value: &str) -> darling::Result<Self> {
        match value {
            "eq" => Ok(Self::Eq),
            "ne" => Ok(Self::Ne),
            "lt" => Ok(Self::Lt),
            "lte" => Ok(Self::Lte),
            "gt" => Ok(Self::Gt),
            "gte" => Ok(Self::Gte),
            "like" => Ok(Self::Like),
            "ilike" => Ok(Self::ILike),
            "in" => Ok(Self::In),
            _ => Err(darling::Error::unknown_value(value)),
        }
    }
}

pub fn derive_filterable(
    input: proc_macro2::TokenStream,
    runtime_path: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let input = match syn::parse2::<DeriveInput>(input) {
        Ok(input) => input,
        Err(err) => return err.to_compile_error(),
    };
    derive_filterable_impl(&input, runtime_path)
}

fn derive_filterable_impl(
    input: &DeriveInput,
    runtime_path: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let ident = &input.ident;
    let crate_path = crate::runtime_path(input, runtime_path);
    let model = match parse_model(input) {
        Ok(model) => model,
        Err(e) => return e.to_compile_error(),
    };
    let fields = match named_fields(input) {
        Ok(fields) => fields,
        Err(e) => return e.to_compile_error(),
    };
    let filter_stmts = match filter_statements(fields, &crate_path) {
        Ok(stmts) => stmts,
        Err(e) => return e.to_compile_error(),
    };
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    quote! {
        impl #impl_generics #crate_path::Filterable for #ident #ty_generics #where_clause {
            type Model = #model;

            fn apply_filter(
                &self,
                mut filter: #crate_path::FilterBuilder<Self::Model>,
            ) -> #crate_path::FilterBuilder<Self::Model> {
                #(#filter_stmts)*
                filter
            }
        }
    }
}

fn named_fields(
    input: &DeriveInput,
) -> Result<&syn::punctuated::Punctuated<syn::Field, syn::Token![,]>, Error> {
    match &input.data {
        syn::Data::Struct(data) => match &data.fields {
            syn::Fields::Named(fields) => Ok(&fields.named),
            _ => Err(Error::new_spanned(
                &input.ident,
                "Filterable supports named structs only",
            )),
        },
        _ => Err(Error::new_spanned(
            &input.ident,
            "Filterable supports structs only",
        )),
    }
}

fn filter_statements(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::Token![,]>,
    crate_path: &proc_macro2::TokenStream,
) -> Result<Vec<proc_macro2::TokenStream>, Error> {
    let mut stmts = Vec::new();
    for field in fields {
        let Some(field_ident) = field.ident.as_ref() else {
            continue;
        };
        let Some(attr) = parse_filter_attr(field)? else {
            continue;
        };
        let column = match attr.column {
            Some(column) => {
                syn::parse_str::<Ident>(&column)
                    .map_err(|_| Error::new(attr.span, "column must be a Rust field identifier"))?;
                Ident::new(&column, attr.span)
            }
            None => field_ident.clone(),
        };
        stmts.push(filter_statement(
            field_ident,
            &field.ty,
            &column,
            attr.op,
            crate_path,
        )?);
    }
    Ok(stmts)
}

fn filter_statement(
    field: &Ident,
    ty: &Type,
    column: &Ident,
    op: FilterOp,
    crate_path: &proc_macro2::TokenStream,
) -> Result<proc_macro2::TokenStream, Error> {
    if op == FilterOp::In {
        return in_filter_statement(field, ty, column, crate_path);
    }
    let method = op.method();
    if option_inner_type(ty).is_some() {
        return Ok(quote! {
            if let Some(value) = &self.#field {
                let predicate = filter.#column.#method(#crate_path::val(value.clone()));
                filter = filter.filter(predicate);
            }
        });
    }
    Ok(quote! {
        let predicate = filter.#column.#method(#crate_path::val(self.#field.clone()));
        filter = filter.filter(predicate);
    })
}

fn in_filter_statement(
    field: &Ident,
    ty: &Type,
    column: &Ident,
    crate_path: &proc_macro2::TokenStream,
) -> Result<proc_macro2::TokenStream, Error> {
    if let Some(inner) = option_inner_type(ty) {
        if vec_inner_type(inner).is_none() {
            return Err(Error::new(
                ty.span(),
                "the in filter operator requires Vec<T> or Option<Vec<T>>",
            ));
        }
        return Ok(quote! {
            if let Some(values) = &self.#field {
                if !values.is_empty() {
                    let predicate = #crate_path::filters::__private::in_values(
                        &filter.#column,
                        values.iter().cloned().map(#crate_path::val),
                    );
                    filter = filter.filter(predicate);
                }
            }
        });
    }
    if vec_inner_type(ty).is_none() {
        return Err(Error::new(
            ty.span(),
            "the in filter operator requires Vec<T> or Option<Vec<T>>",
        ));
    }
    Ok(quote! {
        if !self.#field.is_empty() {
            let predicate = #crate_path::filters::__private::in_values(
                &filter.#column,
                self.#field.iter().cloned().map(#crate_path::val),
            );
            filter = filter.filter(predicate);
        }
    })
}

fn parse_model(input: &DeriveInput) -> Result<Path, Error> {
    let mut model = None;
    for attr in input
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("filter"))
    {
        let nested = parse_nested(attr)?;
        let parsed = FilterContainer::from_list(&nested)
            .map_err(|err| Error::new(attr.span(), err.to_string()))?;
        if model.is_some() && parsed.model.is_some() {
            return Err(Error::new(attr.span(), "model can only be set once"));
        }
        model = model.or(parsed.model);
    }
    model.ok_or_else(|| {
        Error::new_spanned(
            &input.ident,
            "Filterable requires #[filter(model = ModelType)]",
        )
    })
}

fn parse_filter_attr(field: &syn::Field) -> Result<Option<FilterAttr>, Error> {
    let attrs: Vec<_> = field
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("filter"))
        .collect();
    if attrs.is_empty() {
        return Ok(None);
    }
    if attrs.len() > 1 {
        return Err(Error::new(
            attrs[1].span(),
            "only one #[filter(...)] attribute is supported per field",
        ));
    }
    let nested = parse_nested(attrs[0])?;
    let parsed = FilterMeta::from_list(&nested)
        .map_err(|err| Error::new(attrs[0].span(), err.to_string()))?;
    Ok(Some(FilterAttr {
        op: parsed.op.unwrap_or(FilterOp::Eq),
        column: parsed.column,
        span: attrs[0].span(),
    }))
}

fn parse_nested(attr: &syn::Attribute) -> Result<Vec<darling::ast::NestedMeta>, Error> {
    match &attr.meta {
        syn::Meta::List(list) => darling::ast::NestedMeta::parse_meta_list(list.tokens.clone())
            .map_err(|err| Error::new(attr.span(), err.to_string())),
        syn::Meta::Path(_) => Ok(Vec::new()),
        _ => Err(Error::new(attr.span(), "expected #[filter(...)]")),
    }
}

fn option_inner_type(ty: &Type) -> Option<&Type> {
    let Type::Path(path) = ty else {
        return None;
    };
    let segment = path.path.segments.last()?;
    if segment.ident != "Option" {
        return None;
    }
    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return None;
    };
    let Some(GenericArgument::Type(inner)) = args.args.first() else {
        return None;
    };
    Some(inner)
}

fn vec_inner_type(ty: &Type) -> Option<&Type> {
    let Type::Path(path) = ty else {
        return None;
    };
    let segment = path.path.segments.last()?;
    if segment.ident != "Vec" {
        return None;
    }
    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return None;
    };
    let Some(GenericArgument::Type(inner)) = args.args.first() else {
        return None;
    };
    Some(inner)
}
