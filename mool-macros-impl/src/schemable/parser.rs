use darling::FromMeta;
use syn::{
    Attribute, DeriveInput, Error, Expr, ExprArray, Fields, Lit, LitStr, Meta, Result, Type,
    spanned::Spanned,
};

// -------------------------------------------------------------------------------------
// Namespace key allowlists (used only for better error messages; does not change acceptance)
// -------------------------------------------------------------------------------------

static COLUMN_KEYS: &[&str] = &[
    "name",
    "type",
    "nullable",
    "primary_key",
    "serial",
    "skip",
    "flatten",
    "json",
    "sql_enum",
    "reference",
    "backref",
    "prefetch",
    "read_only",
    "skip_bind",
    "selectable",
    "insertable",
    "updatable",
    "default",
    "check",
    "index",
    "index_name",
    "unique",
    "unique_name",
];

// -------------------------------------------------------------------------------------
// Small helpers
// -------------------------------------------------------------------------------------

fn parse_ns_list(attr: &Attribute, ns: &str) -> Result<Option<Vec<darling::ast::NestedMeta>>> {
    if !attr.path().is_ident(ns) {
        return Ok(None);
    }

    let nested = match &attr.meta {
        syn::Meta::List(list) => {
            let tokens = normalize_attr_keywords(list.tokens.clone());
            darling::ast::NestedMeta::parse_meta_list(tokens).map_err(|e| {
                augment_darling_error(e.into(), &format!("Error parsing #[{ns}(...)]"))
            })? // preserves spans
        }
        syn::Meta::Path(_) => Vec::new(),
        _ => {
            return Err(Error::new(
                attr.span(),
                format!("expected #[{ns}] or #[{ns}(...)]"),
            ));
        }
    };

    Ok(Some(nested))
}

fn normalize_attr_keywords(tokens: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    use proc_macro2::{Group, Ident, TokenTree};

    tokens
        .into_iter()
        .map(|token| match token {
            TokenTree::Ident(ident) if ident == "type" => {
                TokenTree::Ident(Ident::new_raw("type", ident.span()))
            }
            TokenTree::Group(group) => {
                let mut out =
                    Group::new(group.delimiter(), normalize_attr_keywords(group.stream()));
                out.set_span(group.span());
                TokenTree::Group(out)
            }
            other => other,
        })
        .collect()
}

fn top_level_key(nm: &darling::ast::NestedMeta) -> Option<&syn::Path> {
    use darling::ast::NestedMeta;
    match nm {
        NestedMeta::Meta(syn::Meta::Path(p)) => Some(p),
        NestedMeta::Meta(syn::Meta::NameValue(nv)) => Some(&nv.path),
        NestedMeta::Meta(syn::Meta::List(ml)) => Some(&ml.path),
        NestedMeta::Lit(_) => None,
    }
}

fn enforce_namespace(
    ns: &str,
    nested: &[darling::ast::NestedMeta],
    allowed: &[&str],
) -> Result<()> {
    for nm in nested {
        let Some(path) = top_level_key(nm) else {
            continue;
        };
        let Some(ident) = path.get_ident() else {
            continue;
        };
        let key = ident.to_string().trim_start_matches("r#").to_string();

        if !allowed.iter().any(|a| *a == key) {
            // "belongs to ..." hint (best-effort)
            let hint = if COLUMN_KEYS.contains(&key.as_str()) {
                " (belongs to #[column(...)] )"
            } else {
                ""
            };

            return Err(Error::new(
                ident.span(),
                format!("Unknown option '{key}' in #[{ns}(...)]{hint}"),
            ));
        }
    }
    Ok(())
}

/// Reference specification for foreign key relationships
#[derive(Debug, Clone, Default)]
pub struct ReferenceSpec {
    pub target: Option<String>,
    pub name: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub join: Option<String>,
    pub on: Vec<JoinOnSpec>,
    pub relation: Option<syn::Path>,
}

#[derive(Debug, Clone, FromMeta)]
pub struct JoinOnSpec {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone)]
pub struct MarkerSpec {
    pub path: syn::Path,
}

impl FromMeta for MarkerSpec {
    fn from_meta(item: &Meta) -> darling::Result<Self> {
        match item {
            Meta::Path(path) => Ok(Self { path: path.clone() }),
            Meta::List(list) => Self::from_list(
                &darling::ast::NestedMeta::parse_meta_list(list.tokens.clone())
                    .map_err(darling::Error::from)?,
            ),
            Meta::NameValue(name_value) => Self::from_expr(&name_value.value),
        }
    }

    fn from_expr(expr: &Expr) -> darling::Result<Self> {
        match expr {
            Expr::Path(path) => Ok(Self {
                path: path.path.clone(),
            }),
            _ => Err(darling::Error::custom("expected marker type").with_span(expr)),
        }
    }

    fn from_list(items: &[darling::ast::NestedMeta]) -> darling::Result<Self> {
        if items.len() != 1 {
            return Err(darling::Error::custom("expected a single marker type"));
        }
        let Some(item) = items.first() else {
            return Err(darling::Error::custom("expected a marker type"));
        };
        match item {
            darling::ast::NestedMeta::Meta(Meta::Path(path)) => Ok(Self { path: path.clone() }),
            darling::ast::NestedMeta::Meta(meta) => {
                Err(darling::Error::custom("expected marker type").with_span(meta))
            }
            darling::ast::NestedMeta::Lit(lit) => {
                Err(darling::Error::custom("expected marker type").with_span(lit))
            }
        }
    }
}

impl FromMeta for ReferenceSpec {
    fn from_word() -> darling::Result<Self> {
        Ok(ReferenceSpec::default())
    }

    fn from_expr(expr: &Expr) -> darling::Result<Self> {
        Ok(ReferenceSpec {
            target: Some(lit_str_value(expr)?),
            ..ReferenceSpec::default()
        })
    }

    fn from_list(items: &[darling::ast::NestedMeta]) -> darling::Result<Self> {
        let mut spec = ReferenceSpec::default();
        for item in items {
            match item {
                darling::ast::NestedMeta::Meta(meta) => parse_reference_meta(meta, &mut spec)?,
                darling::ast::NestedMeta::Lit(lit) => {
                    return Err(
                        darling::Error::custom("unsupported reference literal").with_span(lit)
                    );
                }
            }
        }
        Ok(spec)
    }
}

fn parse_reference_meta(meta: &Meta, spec: &mut ReferenceSpec) -> darling::Result<()> {
    match meta {
        Meta::Path(path) => {
            spec.relation = Some(path.clone());
            Ok(())
        }
        Meta::NameValue(name_value) => parse_reference_name_value(name_value, spec),
        Meta::List(list) if list.path.is_ident("on") => parse_reference_on(list, spec),
        Meta::List(list) => {
            Err(darling::Error::custom("unsupported reference list").with_span(list))
        }
    }
}

fn parse_reference_name_value(
    name_value: &syn::MetaNameValue,
    spec: &mut ReferenceSpec,
) -> darling::Result<()> {
    let value = lit_str_value(&name_value.value)?;
    if name_value.path.is_ident("from") {
        spec.from = Some(value);
        return Ok(());
    }
    if name_value.path.is_ident("to") {
        spec.to = Some(value);
        return Ok(());
    }
    if name_value.path.is_ident("join") {
        spec.join = Some(value);
        return Ok(());
    }
    if name_value.path.is_ident("target") {
        spec.target = Some(value);
        return Ok(());
    }
    if name_value.path.is_ident("name") {
        spec.name = Some(value);
        return Ok(());
    }
    Err(darling::Error::custom("unknown reference argument").with_span(name_value))
}

impl ReferenceSpec {
    pub fn is_join_reference(&self) -> bool {
        self.target.is_none()
    }
}

fn lit_str_value(expr: &Expr) -> darling::Result<String> {
    match expr {
        Expr::Lit(lit) => match &lit.lit {
            Lit::Str(value) => Ok(value.value()),
            _ => Err(darling::Error::custom("expected string literal").with_span(expr)),
        },
        _ => Err(darling::Error::custom("expected string literal").with_span(expr)),
    }
}

fn parse_reference_on(list: &syn::MetaList, spec: &mut ReferenceSpec) -> darling::Result<()> {
    let nested = darling::ast::NestedMeta::parse_meta_list(list.tokens.clone())
        .map_err(darling::Error::from)?;
    let on = JoinOnSpec::from_list(&nested)?;
    spec.on.push(on);
    Ok(())
}

#[derive(Debug, Clone, Default)]
pub struct StringList(pub Vec<String>);

impl StringList {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &String> {
        self.0.iter()
    }
}

impl FromMeta for StringList {
    fn from_expr(expr: &Expr) -> darling::Result<Self> {
        match expr {
            Expr::Array(array) => parse_string_array(array).map(Self),
            Expr::Lit(lit) => parse_string_lit(&lit.lit).map(|value| Self(vec![value])),
            _ => Err(darling::Error::custom("expected string or string array").with_span(expr)),
        }
    }
}

fn parse_string_array(array: &ExprArray) -> darling::Result<Vec<String>> {
    array
        .elems
        .iter()
        .map(|expr| match expr {
            Expr::Lit(lit) => parse_string_lit(&lit.lit),
            _ => Err(darling::Error::custom("expected string literal").with_span(expr)),
        })
        .collect()
}

fn parse_string_lit(lit: &Lit) -> darling::Result<String> {
    match lit {
        Lit::Str(value) => Ok(value.value()),
        _ => Err(darling::Error::custom("expected string literal").with_span(lit)),
    }
}

/// Table-level primary key metadata from `#[table(primary_key(...))]`.
#[derive(Debug, Clone, Default, FromMeta)]
pub struct PrimaryKeySpec {
    #[darling(default)]
    pub name: Option<String>,
    #[darling(default)]
    pub columns: StringList,
}

/// Target side of a table-level composite foreign key.
#[derive(Debug, Clone, FromMeta)]
pub struct ForeignKeyTargetSpec {
    pub table: String,
    pub columns: StringList,
}

/// Table-level foreign key metadata from `#[table(foreign_key(...))]`.
#[derive(Debug, Clone, FromMeta)]
pub struct ForeignKeySpec {
    #[darling(default)]
    pub name: Option<String>,
    pub columns: StringList,
    pub references: ForeignKeyTargetSpec,
}

// -------------------------------------------------------------------------------------
// Attributes
// -------------------------------------------------------------------------------------

/// Container-level schema attributes
#[derive(Debug, Default, Clone, FromMeta)]
pub struct ContainerAttrs {
    #[darling(default)]
    pub name: Option<LitStr>,
    #[darling(default)]
    pub table: Option<LitStr>,
    #[darling(default)]
    pub schema: Option<LitStr>,
    #[darling(default)]
    pub primary_key: Option<PrimaryKeySpec>,
    #[darling(default, multiple, rename = "foreign_key")]
    pub foreign_keys: Vec<ForeignKeySpec>,
}

/// Database column metadata from #[column(...)]
#[derive(Debug, Default, Clone, FromMeta)]
pub struct ColumnAttrs {
    #[darling(default)]
    pub name: Option<LitStr>,

    #[darling(default, rename = "r#type")]
    pub sql_type: Option<LitStr>,
    #[darling(default)]
    pub nullable: Option<bool>,

    #[darling(default)]
    pub primary_key: bool,
    #[darling(default)]
    pub serial: bool,

    #[darling(default)]
    pub skip: bool,
    #[darling(default)]
    pub flatten: bool,
    #[darling(default)]
    pub json: bool,
    #[darling(default)]
    pub sql_enum: bool,
    #[darling(default)]
    pub reference: Option<ReferenceSpec>,
    #[darling(default)]
    pub backref: Option<MarkerSpec>,
    #[darling(default)]
    pub prefetch: Option<MarkerSpec>,

    #[darling(default)]
    pub read_only: bool,
    #[darling(default)]
    pub skip_bind: bool,

    #[darling(default)]
    pub selectable: Option<bool>,
    #[darling(default)]
    pub insertable: Option<bool>,
    #[darling(default)]
    pub updatable: Option<bool>,

    #[darling(default)]
    pub default: Option<LitStr>,
    #[darling(default)]
    pub check: Option<LitStr>,

    #[darling(default)]
    pub index: bool,
    #[darling(default)]
    pub index_name: Option<LitStr>,

    #[darling(default)]
    pub unique: bool,
    #[darling(default)]
    pub unique_name: Option<LitStr>,
}

/// Combined field attributes across all namespaces
#[derive(Debug, Clone)]
pub struct FieldMeta {
    pub ident: Option<syn::Ident>,
    pub ty: Type,
    pub column: ColumnAttrs,
}

fn augment_darling_error(e: darling::Error, context: &str) -> syn::Error {
    let mut combined_err: Option<syn::Error> = None;

    for single_err in e {
        let syn_err: syn::Error = single_err.into();
        let msg = format!("{}: {}", context, syn_err);
        let enriched = Error::new(syn_err.span(), msg);
        match combined_err {
            Some(ref mut existing) => existing.combine(enriched),
            None => combined_err = Some(enriched),
        }
    }

    combined_err.unwrap_or_else(|| {
        Error::new(
            proc_macro2::Span::call_site(),
            format!("{}: Unknown error", context),
        )
    })
}

impl FieldMeta {
    pub fn from_field(field: &syn::Field) -> Result<Self> {
        let mut column = ColumnAttrs::default();

        let field_name = field
            .ident
            .as_ref()
            .map(|i| i.to_string())
            .unwrap_or_else(|| "<unnamed>".to_string());

        for attr in &field.attrs {
            if let Some(nested) = parse_ns_list(attr, "column")? {
                enforce_namespace("column", &nested, COLUMN_KEYS)?;
                column = ColumnAttrs::from_list(&nested).map_err(|e| {
                    augment_darling_error(
                        e,
                        &format!("Error decoding #[column] on field '{field_name}'"),
                    )
                })?;
                if column.sql_enum && column.sql_type.is_some() {
                    return Err(Error::new(
                        attr.span(),
                        "column sql_enum cannot be combined with type",
                    ));
                }
            }
        }

        Ok(Self {
            ident: field.ident.clone(),
            ty: field.ty.clone(),
            column,
        })
    }
}

impl ContainerAttrs {
    pub fn from_attrs(attrs: &[Attribute]) -> Result<Self> {
        let mut out = Self::default();
        for attr in attrs {
            if let Some(nested) = parse_ns_list(attr, "table")? {
                out = ContainerAttrs::from_list(&nested)
                    .map_err(|e| augment_darling_error(e, "Error decoding #[table]"))?;
            }
        }
        Ok(out)
    }
}

pub fn to_snake_case(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    for (i, ch) in s.chars().enumerate() {
        if ch.is_uppercase() && i != 0 {
            out.push('_');
        }
        out.push(ch.to_lowercase().next().unwrap_or(ch));
    }
    out
}

/// Parsed struct information ready for codegen
#[derive(Debug)]
pub struct ParsedStruct {
    pub ident: syn::Ident,
    pub generics: syn::Generics,
    pub container: ContainerAttrs,
    pub fields: Vec<FieldMeta>,
}

impl ParsedStruct {
    pub fn from_derive_input(input: DeriveInput) -> Result<Self> {
        let container = ContainerAttrs::from_attrs(&input.attrs)?;
        let ident = input.ident;
        let generics = input.generics;

        let fields = match input.data {
            syn::Data::Struct(data) => match data.fields {
                Fields::Named(fields) => fields
                    .named
                    .iter()
                    .map(FieldMeta::from_field)
                    .collect::<Result<Vec<_>>>()?,
                _ => {
                    return Err(Error::new_spanned(
                        ident,
                        "only structs with named fields are supported",
                    ));
                }
            },
            _ => return Err(Error::new_spanned(ident, "only structs are supported")),
        };
        validate_semantics(&container, &fields, &ident)?;
        Ok(Self {
            ident,
            generics,
            container,
            fields,
        })
    }
}

fn validate_semantics(
    container: &ContainerAttrs,
    fields: &[FieldMeta],
    ident: &syn::Ident,
) -> Result<()> {
    validate_container_identifiers(container, ident)?;
    validate_fields(fields)?;
    validate_table_constraints(container, fields, ident)
}

fn validate_container_identifiers(container: &ContainerAttrs, ident: &syn::Ident) -> Result<()> {
    for value in [
        container.name.as_ref(),
        container.table.as_ref(),
        container.schema.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        validate_identifier(&value.value(), value.span())?;
    }
    let table_name = container.name.as_ref().or(container.table.as_ref());
    if container.name.is_some() && container.table.is_some() {
        return Err(Error::new_spanned(
            ident,
            "table name cannot be specified twice",
        ));
    }
    if table_name.is_none() {
        validate_identifier(&to_snake_case(&ident.to_string()), ident.span())?;
    }
    Ok(())
}

fn validate_fields(fields: &[FieldMeta]) -> Result<()> {
    let mut columns = std::collections::HashSet::new();
    let mut relations = std::collections::HashSet::new();
    for field in fields {
        validate_field_flags(field)?;
        validate_field_names(field)?;
        if is_physical_column(field) {
            let name = effective_column_name(field);
            if !columns.insert(name.clone()) {
                return Err(Error::new(
                    field_span(field),
                    format!("duplicate column '{name}'"),
                ));
            }
        }
        if is_relation_field(field) {
            let name = field
                .ident
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_default();
            if !relations.insert(name.clone()) {
                return Err(Error::new(
                    field_span(field),
                    format!("duplicate relation alias '{name}'"),
                ));
            }
        }
    }
    Ok(())
}

/// Rejects write-policy and relation combinations with contradictory behavior.
fn validate_field_flags(field: &FieldMeta) -> Result<()> {
    let column = &field.column;
    validate_field_storage_flags(field)?;
    if column.read_only && (column.insertable == Some(true) || column.updatable == Some(true)) {
        return Err(Error::new(
            field_span(field),
            "read_only cannot be insertable or updatable",
        ));
    }
    if column.skip_bind && (column.insertable == Some(true) || column.updatable == Some(true)) {
        return Err(Error::new(
            field_span(field),
            "skip_bind cannot be insertable or updatable",
        ));
    }
    let relation_count = usize::from(column.reference.is_some())
        + usize::from(column.backref.is_some())
        + usize::from(column.prefetch.is_some());
    if relation_count > 1 {
        return Err(Error::new(
            field_span(field),
            "reference, backref, and prefetch are mutually exclusive",
        ));
    }
    validate_reference_spec(field)
}

/// Rejects storage metadata that cannot describe one physical column safely.
fn validate_field_storage_flags(field: &FieldMeta) -> Result<()> {
    let column = &field.column;
    if column.skip
        && (column.primary_key
            || column.flatten
            || column.reference.is_some()
            || column.backref.is_some()
            || column.prefetch.is_some())
    {
        return Err(Error::new(
            field_span(field),
            "skip cannot be combined with primary-key or relation metadata",
        ));
    }
    if column.primary_key && column.nullable == Some(true) {
        return Err(Error::new(
            field_span(field),
            "primary-key columns cannot be nullable",
        ));
    }
    if column.json && column.sql_enum {
        return Err(Error::new(
            field_span(field),
            "json and sql_enum storage are mutually exclusive",
        ));
    }
    if column.flatten
        && (column.name.is_some() || column.sql_type.is_some() || column.json || column.sql_enum)
    {
        return Err(Error::new(
            field_span(field),
            "flatten cannot define column storage metadata",
        ));
    }
    Ok(())
}

fn validate_field_names(field: &FieldMeta) -> Result<()> {
    if let Some(name) = &field.column.name {
        validate_identifier(&name.value(), name.span())?;
    }
    for name in [
        field.column.index_name.as_ref(),
        field.column.unique_name.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        validate_identifier(&name.value(), name.span())?;
    }
    Ok(())
}

fn validate_reference_spec(field: &FieldMeta) -> Result<()> {
    let Some(reference) = &field.column.reference else {
        return Ok(());
    };
    if let Some(target) = &reference.target {
        let Some((table, column)) = target.rsplit_once('.') else {
            return Err(Error::new(
                field_span(field),
                "reference target must be 'table.column'",
            ));
        };
        validate_qualified_identifier(table, field_span(field))?;
        validate_identifier(column, field_span(field))?;
    } else if reference.relation.is_none() && reference.on.is_empty() && reference.from.is_none() {
        return Err(Error::new(
            field_span(field),
            "join reference requires from, on(...), or a relation marker",
        ));
    }
    if let Some(join) = &reference.join
        && !matches!(join.as_str(), "inner" | "left")
    {
        return Err(Error::new(
            field_span(field),
            "reference join must be 'inner' or 'left'",
        ));
    }
    for join in &reference.on {
        validate_qualified_identifier(&join.from, field_span(field))?;
        validate_identifier(&join.to, field_span(field))?;
    }
    Ok(())
}

fn validate_table_constraints(
    container: &ContainerAttrs,
    fields: &[FieldMeta],
    ident: &syn::Ident,
) -> Result<()> {
    let columns = fields
        .iter()
        .filter(|field| is_physical_column(field))
        .map(effective_column_name)
        .collect::<std::collections::HashSet<_>>();
    if let Some(primary_key) = &container.primary_key {
        validate_constraint_columns(
            "primary key",
            primary_key.columns.iter(),
            &columns,
            ident.span(),
        )?;
        if let Some(name) = &primary_key.name {
            validate_identifier(name, ident.span())?;
        }
    }
    for foreign_key in &container.foreign_keys {
        validate_foreign_key(foreign_key, &columns, ident.span())?;
    }
    Ok(())
}

fn validate_foreign_key(
    foreign_key: &ForeignKeySpec,
    columns: &std::collections::HashSet<String>,
    span: proc_macro2::Span,
) -> Result<()> {
    validate_constraint_columns("foreign key", foreign_key.columns.iter(), columns, span)?;
    if foreign_key.columns.len() != foreign_key.references.columns.len() {
        return Err(Error::new(
            span,
            "foreign-key source and target arity must match",
        ));
    }
    validate_qualified_identifier(&foreign_key.references.table, span)?;
    for column in foreign_key.references.columns.iter() {
        validate_identifier(column, span)?;
    }
    if let Some(name) = &foreign_key.name {
        validate_identifier(name, span)?;
    }
    Ok(())
}

fn validate_constraint_columns<'a>(
    label: &str,
    columns: impl Iterator<Item = &'a String>,
    known: &std::collections::HashSet<String>,
    span: proc_macro2::Span,
) -> Result<()> {
    let columns = columns.collect::<Vec<_>>();
    if columns.is_empty() {
        return Err(Error::new(
            span,
            format!("{label} requires at least one column"),
        ));
    }
    for column in columns {
        validate_identifier(column, span)?;
        if !known.contains(column) {
            return Err(Error::new(
                span,
                format!("{label} references unknown column '{column}'"),
            ));
        }
    }
    Ok(())
}

fn validate_qualified_identifier(value: &str, span: proc_macro2::Span) -> Result<()> {
    for part in value.split('.') {
        validate_identifier(part, span)?;
    }
    Ok(())
}

fn validate_identifier(value: &str, span: proc_macro2::Span) -> Result<()> {
    let mut chars = value.chars();
    let valid_start = chars
        .next()
        .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_');
    if valid_start && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_') {
        return Ok(());
    }
    Err(Error::new(
        span,
        format!("invalid SQL identifier '{value}'"),
    ))
}

fn effective_column_name(field: &FieldMeta) -> String {
    field
        .column
        .name
        .as_ref()
        .map(LitStr::value)
        .or_else(|| field.ident.as_ref().map(ToString::to_string))
        .unwrap_or_default()
}

fn is_physical_column(field: &FieldMeta) -> bool {
    !field.column.skip && !field.column.flatten && field.column.prefetch.is_none()
}

fn is_relation_field(field: &FieldMeta) -> bool {
    field.column.reference.is_some()
        || field.column.backref.is_some()
        || field.column.prefetch.is_some()
}

fn field_span(field: &FieldMeta) -> proc_macro2::Span {
    field
        .ident
        .as_ref()
        .map(syn::Ident::span)
        .unwrap_or_else(|| field.ty.span())
}
