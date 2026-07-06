//! Parsed model for `SqlEnum` code generation.

use std::collections::HashSet;

use syn::{Data, DeriveInput, Error, Fields, Ident, Result};

use super::attrs::{IntRepr, Storage, parse_enum_attrs, parse_variant_attrs};
use super::rename::{RenameRule, default_sql_name};

#[derive(Debug)]
pub struct ParsedSqlEnum {
    pub ident: Ident,
    pub storage: Storage,
    pub repr: IntRepr,
    pub sql_name: String,
    pub variants: Vec<SqlEnumVariant>,
}

#[derive(Debug)]
pub struct SqlEnumVariant {
    pub ident: Ident,
    pub label: String,
    pub code: Option<i64>,
}

impl ParsedSqlEnum {
    pub fn from_input(input: &DeriveInput) -> Result<Self> {
        let enum_attrs = parse_enum_attrs(&input.attrs)?;
        let storage = enum_attrs.storage.unwrap_or(Storage::Text);
        validate_container_attrs(storage, enum_attrs.repr)?;
        let repr = enum_attrs.repr.unwrap_or(IntRepr::I32);
        let rename_all = enum_attrs.rename_all.unwrap_or(RenameRule::Snake);
        let sql_name = enum_attrs
            .name
            .unwrap_or_else(|| default_sql_name(&input.ident.to_string()));
        let variants = parse_variants(input, storage, rename_all)?;
        validate_variants(&variants, storage)?;
        Ok(Self {
            ident: input.ident.clone(),
            storage,
            repr,
            sql_name,
            variants,
        })
    }
}

fn validate_container_attrs(storage: Storage, repr: Option<IntRepr>) -> Result<()> {
    if repr.is_some() && storage != Storage::Int {
        return Err(Error::new(
            proc_macro2::Span::call_site(),
            "repr is only valid with storage = \"int\"",
        ));
    }
    Ok(())
}

fn parse_variants(
    input: &DeriveInput,
    storage: Storage,
    rename_all: RenameRule,
) -> Result<Vec<SqlEnumVariant>> {
    let Data::Enum(data) = &input.data else {
        return Err(Error::new_spanned(
            &input.ident,
            "SqlEnum can only be derived for enums",
        ));
    };
    if data.variants.is_empty() {
        return Err(Error::new_spanned(
            &input.ident,
            "SqlEnum requires at least one variant",
        ));
    }
    data.variants
        .iter()
        .map(|variant| {
            if !matches!(variant.fields, Fields::Unit) {
                return Err(Error::new_spanned(
                    variant,
                    "SqlEnum supports fieldless variants only",
                ));
            }
            let attrs = parse_variant_attrs(&variant.attrs)?;
            validate_variant_attrs(storage, attrs.value.as_ref(), attrs.code)?;
            let label = attrs
                .value
                .unwrap_or_else(|| rename_all.apply(&variant.ident.to_string()));
            Ok(SqlEnumVariant {
                ident: variant.ident.clone(),
                label,
                code: attrs.code,
            })
        })
        .collect()
}

fn validate_variant_attrs(
    storage: Storage,
    value: Option<&String>,
    code: Option<i64>,
) -> Result<()> {
    if code.is_some() && storage != Storage::Int {
        return Err(Error::new(
            proc_macro2::Span::call_site(),
            "code is only valid with storage = \"int\"",
        ));
    }
    if value.is_some() && storage == Storage::Int {
        return Err(Error::new(
            proc_macro2::Span::call_site(),
            "value is not valid with storage = \"int\"",
        ));
    }
    Ok(())
}

fn validate_variants(variants: &[SqlEnumVariant], storage: Storage) -> Result<()> {
    let mut labels = HashSet::new();
    for variant in variants {
        if !labels.insert(variant.label.as_str()) {
            return Err(Error::new_spanned(
                &variant.ident,
                format!("duplicate SQL enum label '{}'", variant.label),
            ));
        }
    }
    if storage == Storage::Int {
        let mut codes = HashSet::new();
        for variant in variants {
            let Some(code) = variant.code else {
                return Err(Error::new_spanned(
                    &variant.ident,
                    "storage = \"int\" requires every variant to define code",
                ));
            };
            if !codes.insert(code) {
                return Err(Error::new_spanned(
                    &variant.ident,
                    format!("duplicate SQL enum code '{code}'"),
                ));
            }
        }
    }
    Ok(())
}
