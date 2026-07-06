//! Attribute parsing for `SqlEnum`.

use syn::{Attribute, Error, LitInt, LitStr, Result, spanned::Spanned};

use super::rename::RenameRule;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Storage {
    Text,
    Int,
    NativePostgres,
    NativeMysql,
}

impl Storage {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "text" => Some(Self::Text),
            "int" => Some(Self::Int),
            "native_postgres" => Some(Self::NativePostgres),
            "native_mysql" => Some(Self::NativeMysql),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntRepr {
    I16,
    I32,
    I64,
}

impl IntRepr {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "i16" => Some(Self::I16),
            "i32" => Some(Self::I32),
            "i64" => Some(Self::I64),
            _ => None,
        }
    }
}

#[derive(Debug, Default)]
pub struct EnumAttrs {
    pub name: Option<String>,
    pub storage: Option<Storage>,
    pub rename_all: Option<RenameRule>,
    pub repr: Option<IntRepr>,
}

#[derive(Debug, Default)]
pub struct VariantAttrs {
    pub value: Option<String>,
    pub code: Option<i64>,
}

pub fn parse_enum_attrs(attrs: &[Attribute]) -> Result<EnumAttrs> {
    let mut out = EnumAttrs::default();
    for attr in attrs.iter().filter(|attr| attr.path().is_ident("sql_enum")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("name") {
                ensure_unset(out.name.is_some(), &meta, "name")?;
                let value: LitStr = meta.value()?.parse()?;
                out.name = Some(value.value());
                return Ok(());
            }
            if meta.path.is_ident("storage") {
                ensure_unset(out.storage.is_some(), &meta, "storage")?;
                let value: LitStr = meta.value()?.parse()?;
                let Some(storage) = Storage::parse(&value.value()) else {
                    return Err(meta.error("storage must be text, int, native_postgres, or native_mysql"));
                };
                out.storage = Some(storage);
                return Ok(());
            }
            if meta.path.is_ident("rename_all") {
                ensure_unset(out.rename_all.is_some(), &meta, "rename_all")?;
                let value: LitStr = meta.value()?.parse()?;
                let Some(rule) = RenameRule::parse(&value.value()) else {
                    return Err(meta.error(
                        "rename_all must be snake_case, kebab-case, lowercase, UPPERCASE, PascalCase, or camelCase",
                    ));
                };
                out.rename_all = Some(rule);
                return Ok(());
            }
            if meta.path.is_ident("repr") {
                ensure_unset(out.repr.is_some(), &meta, "repr")?;
                let value: LitStr = meta.value()?.parse()?;
                let Some(repr) = IntRepr::parse(&value.value()) else {
                    return Err(meta.error("repr must be i16, i32, or i64"));
                };
                out.repr = Some(repr);
                return Ok(());
            }
            Err(meta.error("unsupported sql_enum attribute"))
        })?;
    }
    Ok(out)
}

pub fn parse_variant_attrs(attrs: &[Attribute]) -> Result<VariantAttrs> {
    let mut out = VariantAttrs::default();
    for attr in attrs.iter().filter(|attr| attr.path().is_ident("sql_enum")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("value") {
                ensure_unset(out.value.is_some(), &meta, "value")?;
                let value: LitStr = meta.value()?.parse()?;
                out.value = Some(value.value());
                return Ok(());
            }
            if meta.path.is_ident("code") {
                ensure_unset(out.code.is_some(), &meta, "code")?;
                let value: LitInt = meta.value()?.parse()?;
                out.code = Some(value.base10_parse()?);
                return Ok(());
            }
            Err(meta.error("unsupported sql_enum variant attribute"))
        })?;
    }
    Ok(out)
}

fn ensure_unset(
    already_set: bool,
    meta: &syn::meta::ParseNestedMeta<'_>,
    name: &str,
) -> Result<()> {
    if already_set {
        return Err(Error::new(
            meta.path.span(),
            format!("{name} can only be set once"),
        ));
    }
    Ok(())
}
