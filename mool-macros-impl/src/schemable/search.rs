//! Search-specific derive grammar and semantic validation.

use darling::FromMeta;
use syn::{Error, Result, Type};
#[cfg(feature = "vector")]
use syn::{Expr, Lit};

#[cfg(any(feature = "text-search", feature = "fuzzy-search"))]
use super::parser::is_physical_column;
use super::parser::{
    ContainerAttrs, FieldMeta, effective_column_name, field_span, validate_identifier,
};
#[cfg(feature = "text-search")]
use super::parser::{StringList, validate_qualified_identifier};

/// PostgreSQL configuration for a full-text definition.
#[cfg(feature = "text-search")]
#[derive(Debug, Clone, Default, FromMeta)]
pub struct PostgresFullTextSpec {
    #[darling(default)]
    pub config: Option<String>,
}

/// Named table-level full-text metadata.
#[cfg(feature = "text-search")]
#[derive(Debug, Clone, FromMeta)]
pub struct FullTextSpec {
    pub name: String,
    pub columns: StringList,
    #[darling(default)]
    pub index_name: Option<String>,
    #[darling(default)]
    pub postgres: Option<PostgresFullTextSpec>,
}

/// PostgreSQL trigram metadata for one text column.
#[cfg(feature = "fuzzy-search")]
#[derive(Debug, Clone, Default, FromMeta)]
pub struct FuzzySpec {
    #[darling(default)]
    pub index: Option<String>,
    #[darling(default)]
    pub index_name: Option<String>,
}

/// PostgreSQL ANN-index tuning for a vector column.
#[cfg(feature = "vector")]
#[derive(Debug, Clone, Default, FromMeta)]
pub struct PostgresVectorSpec {
    #[darling(default)]
    pub m: Option<u16>,
    #[darling(default)]
    pub ef_construction: Option<u16>,
    #[darling(default)]
    pub lists: Option<u32>,
}

/// MariaDB ANN-index tuning for a vector column.
#[cfg(feature = "vector")]
#[derive(Debug, Clone, Default, FromMeta)]
pub struct MariaDbVectorSpec {
    #[darling(default)]
    pub m: Option<u16>,
}

/// Vector storage and optional ANN-index metadata for one field.
#[cfg(feature = "vector")]
#[derive(Debug, Clone, Default, FromMeta)]
pub struct VectorSpec {
    #[darling(default)]
    pub dimensions: Option<VectorDimensionsSpec>,
    #[darling(default)]
    pub metric: Option<String>,
    #[darling(default)]
    pub index: Option<String>,
    #[darling(default)]
    pub index_name: Option<String>,
    #[darling(default)]
    pub postgres: Option<PostgresVectorSpec>,
    #[darling(default)]
    pub mariadb: Option<MariaDbVectorSpec>,
}

/// Literal vector dimension parsed from a column attribute.
#[cfg(feature = "vector")]
#[derive(Debug, Clone, Copy)]
pub struct VectorDimensionsSpec(pub usize);

#[cfg(feature = "vector")]
impl FromMeta for VectorDimensionsSpec {
    fn from_expr(expr: &Expr) -> darling::Result<Self> {
        let Expr::Lit(literal) = expr else {
            return Err(darling::Error::custom("expected an integer literal").with_span(expr));
        };
        let Lit::Int(value) = &literal.lit else {
            return Err(darling::Error::custom("expected an integer literal").with_span(expr));
        };
        value
            .base10_parse()
            .map(Self)
            .map_err(|error| darling::Error::custom(error.to_string()).with_span(value))
    }
}

pub(super) fn validate_search_metadata(
    container: &ContainerAttrs,
    fields: &[FieldMeta],
    ident: &syn::Ident,
) -> Result<()> {
    #[cfg(not(feature = "text-search"))]
    let _ = (container, ident);
    let mut identities = std::collections::HashSet::new();
    #[cfg(feature = "text-search")]
    validate_full_text_specs(container, fields, ident, &mut identities)?;
    #[cfg(feature = "fuzzy-search")]
    validate_fuzzy_specs(fields, &mut identities)?;
    #[cfg(feature = "vector")]
    validate_vector_specs(fields, &mut identities)?;
    Ok(())
}

#[cfg(feature = "text-search")]
fn validate_full_text_specs(
    container: &ContainerAttrs,
    fields: &[FieldMeta],
    ident: &syn::Ident,
    identities: &mut std::collections::HashSet<String>,
) -> Result<()> {
    for definition in &container.full_text {
        validate_full_text_identity(definition, ident, identities)?;
        validate_full_text_columns(definition, fields, ident)?;
        if let Some(index_name) = &definition.index_name {
            validate_identifier(index_name, ident.span())?;
        }
        if let Some(config) = definition
            .postgres
            .as_ref()
            .and_then(|value| value.config.as_deref())
        {
            validate_qualified_identifier(config, ident.span())?;
        }
    }
    Ok(())
}

#[cfg(feature = "text-search")]
fn validate_full_text_identity(
    definition: &FullTextSpec,
    ident: &syn::Ident,
    identities: &mut std::collections::HashSet<String>,
) -> Result<()> {
    validate_identifier(&definition.name, ident.span())?;
    if identities.insert(definition.name.clone()) {
        return Ok(());
    }
    Err(Error::new(
        ident.span(),
        format!("duplicate search definition '{}'", definition.name),
    ))
}

#[cfg(feature = "text-search")]
fn validate_full_text_columns(
    definition: &FullTextSpec,
    fields: &[FieldMeta],
    ident: &syn::Ident,
) -> Result<()> {
    if definition.columns.0.is_empty() {
        return Err(Error::new(
            ident.span(),
            "full_text requires at least one column",
        ));
    }
    let mut seen = std::collections::HashSet::new();
    for column in definition.columns.iter() {
        validate_full_text_column(definition, fields, ident, column, &mut seen)?;
    }
    Ok(())
}

#[cfg(feature = "text-search")]
fn validate_full_text_column<'a>(
    definition: &FullTextSpec,
    fields: &'a [FieldMeta],
    ident: &syn::Ident,
    column: &'a str,
    seen: &mut std::collections::HashSet<&'a str>,
) -> Result<()> {
    validate_identifier(column, ident.span())?;
    if !seen.insert(column) {
        return Err(Error::new(
            ident.span(),
            format!("full_text '{}' repeats column '{column}'", definition.name),
        ));
    }
    let field = fields
        .iter()
        .find(|field| effective_column_name(field) == column && is_physical_column(field))
        .ok_or_else(|| {
            Error::new(
                ident.span(),
                format!(
                    "full_text '{}' references unknown column '{column}'",
                    definition.name
                ),
            )
        })?;
    if is_string_type(&field.ty, true) {
        return Ok(());
    }
    Err(Error::new(
        field_span(field),
        format!("full_text column '{column}' must be String or Option<String>"),
    ))
}

#[cfg(feature = "fuzzy-search")]
fn validate_fuzzy_specs(
    fields: &[FieldMeta],
    identities: &mut std::collections::HashSet<String>,
) -> Result<()> {
    for field in fields.iter().filter(|field| field.column.fuzzy.is_some()) {
        let name = effective_column_name(field);
        if !is_physical_column(field) || !is_string_type(&field.ty, false) {
            return Err(Error::new(
                field_span(field),
                "fuzzy search requires a non-null String column",
            ));
        }
        validate_column_identity(field, &name, identities)?;
        validate_fuzzy_options(field)?;
    }
    Ok(())
}

#[cfg(feature = "fuzzy-search")]
fn validate_fuzzy_options(field: &FieldMeta) -> Result<()> {
    let Some(spec) = field.column.fuzzy.as_ref() else {
        return Ok(());
    };
    if let Some(index) = &spec.index
        && !matches!(index.as_str(), "none" | "gist" | "gin")
    {
        return Err(Error::new(
            field_span(field),
            "fuzzy index must be 'none', 'gist', or 'gin'",
        ));
    }
    if let Some(index_name) = &spec.index_name {
        validate_identifier(index_name, field_span(field))?;
    }
    Ok(())
}

#[cfg(feature = "vector")]
fn validate_vector_specs(
    fields: &[FieldMeta],
    identities: &mut std::collections::HashSet<String>,
) -> Result<()> {
    for field in fields
        .iter()
        .filter(|field| field.column.vector.is_some() || is_vector_type(&field.ty))
    {
        let name = effective_column_name(field);
        if !is_vector_type(&field.ty) {
            return Err(Error::new(
                field_span(field),
                "vector configuration requires a non-null Vector column",
            ));
        }
        if field
            .column
            .vector
            .as_ref()
            .and_then(|spec| spec.dimensions.map(|dimensions| dimensions.0))
            == Some(0)
        {
            return Err(Error::new(
                field_span(field),
                "vector dimension must be greater than zero",
            ));
        }
        validate_column_identity(field, &name, identities)?;
        validate_vector_options(field)?;
    }
    Ok(())
}

#[cfg(any(feature = "fuzzy-search", feature = "vector"))]
fn validate_column_identity(
    field: &FieldMeta,
    name: &str,
    identities: &mut std::collections::HashSet<String>,
) -> Result<()> {
    if identities.insert(name.to_string()) {
        return Ok(());
    }
    Err(Error::new(
        field_span(field),
        format!("duplicate search definition '{name}'"),
    ))
}

#[cfg(feature = "vector")]
fn validate_vector_options(field: &FieldMeta) -> Result<()> {
    let Some(spec) = field.column.vector.as_ref() else {
        return Ok(());
    };
    let metric = spec.metric.as_deref().unwrap_or("cosine");
    let index = spec.index.as_deref().unwrap_or("none");
    validate_vector_kind(field, metric, index)?;
    if index != "none" && spec.dimensions.is_none() {
        return Err(vector_error(
            field,
            "vector ANN indexes require dimensions = ...",
        ));
    }
    validate_postgres_vector_options(field, spec, index)?;
    validate_mariadb_vector_options(field, spec, index)?;
    if let Some(index_name) = &spec.index_name {
        validate_identifier(index_name, field_span(field))?;
    }
    Ok(())
}

#[cfg(feature = "vector")]
fn validate_vector_kind(field: &FieldMeta, metric: &str, index: &str) -> Result<()> {
    if !matches!(metric, "cosine" | "l2") {
        return Err(Error::new(
            field_span(field),
            "vector metric must be 'cosine' or 'l2'",
        ));
    }
    if !matches!(index, "none" | "hnsw" | "ivfflat") {
        return Err(Error::new(
            field_span(field),
            "vector index must be 'none', 'hnsw', or 'ivfflat'",
        ));
    }
    Ok(())
}

#[cfg(feature = "vector")]
fn validate_postgres_vector_options(
    field: &FieldMeta,
    spec: &VectorSpec,
    index: &str,
) -> Result<()> {
    let lists = spec.postgres.as_ref().and_then(|value| value.lists);
    if index == "ivfflat" && lists.is_none() {
        return Err(vector_error(
            field,
            "ivfflat requires postgres(lists = ...)",
        ));
    }
    if index != "ivfflat" && lists.is_some() {
        return Err(vector_error(
            field,
            "postgres lists is only valid for ivfflat",
        ));
    }
    validate_postgres_index_family(field, spec, index)?;
    if lists.is_some_and(|lists| lists == 0) {
        return Err(vector_error(
            field,
            "postgres lists must be greater than zero",
        ));
    }
    Ok(())
}

#[cfg(feature = "vector")]
fn validate_postgres_index_family(field: &FieldMeta, spec: &VectorSpec, index: &str) -> Result<()> {
    if index == "ivfflat"
        && spec
            .postgres
            .as_ref()
            .is_some_and(|value| value.m.is_some() || value.ef_construction.is_some())
    {
        return Err(vector_error(
            field,
            "postgres m and ef_construction are only valid for hnsw",
        ));
    }
    if index == "none" && (spec.postgres.is_some() || spec.mariadb.is_some()) {
        return Err(vector_error(
            field,
            "vector index options require an ANN index",
        ));
    }
    Ok(())
}

#[cfg(feature = "vector")]
fn validate_mariadb_vector_options(
    field: &FieldMeta,
    spec: &VectorSpec,
    index: &str,
) -> Result<()> {
    if index == "ivfflat" && spec.mariadb.is_some() {
        return Err(vector_error(
            field,
            "MariaDB options are incompatible with PostgreSQL-only ivfflat",
        ));
    }
    if let Some(m) = spec.mariadb.as_ref().and_then(|value| value.m)
        && !(3..=200).contains(&m)
    {
        return Err(vector_error(
            field,
            "MariaDB vector m must be between 3 and 200",
        ));
    }
    Ok(())
}

#[cfg(feature = "vector")]
fn vector_error(field: &FieldMeta, message: &str) -> Error {
    Error::new(field_span(field), message)
}

#[cfg(any(feature = "text-search", feature = "fuzzy-search"))]
fn is_string_type(ty: &Type, allow_option: bool) -> bool {
    let candidate = if allow_option {
        option_type_inner(ty).unwrap_or(ty)
    } else {
        ty
    };
    let Type::Path(path) = candidate else {
        return false;
    };
    path.qself.is_none()
        && path.path.segments.last().is_some_and(|segment| {
            segment.ident == "String" && matches!(segment.arguments, syn::PathArguments::None)
        })
}

#[cfg(any(feature = "text-search", feature = "fuzzy-search"))]
fn option_type_inner(ty: &Type) -> Option<&Type> {
    let Type::Path(path) = ty else {
        return None;
    };
    let segment = path.path.segments.last()?;
    if segment.ident != "Option" {
        return None;
    }
    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    match arguments.args.first()? {
        syn::GenericArgument::Type(inner) => Some(inner),
        _ => None,
    }
}

#[cfg(feature = "vector")]
pub fn is_vector_type(ty: &Type) -> bool {
    let Type::Path(path) = ty else {
        return false;
    };
    path.qself.is_none()
        && path.path.segments.last().is_some_and(|segment| {
            segment.ident == "Vector" && matches!(segment.arguments, syn::PathArguments::None)
        })
}
