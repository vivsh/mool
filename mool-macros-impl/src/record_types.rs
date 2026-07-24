//! Rust type-shape and field-policy helpers shared by record code generation.

use syn::{GenericArgument, PathArguments, Type, TypePath};

use crate::schemable::FieldMeta;

/// Returns whether a field is absent from row and write metadata.
pub(super) fn is_skip(field: &FieldMeta) -> bool {
    field.column.skip || field.column.prefetch.is_some()
}

/// Returns whether a field flattens another record into this record.
pub(super) fn is_flatten(field: &FieldMeta) -> bool {
    field.column.flatten
}

/// Returns whether a field represents a joined or back-reference record.
pub(super) fn is_reference(field: &FieldMeta) -> bool {
    field
        .column
        .reference
        .as_ref()
        .is_some_and(|reference| reference.is_join_reference())
        || field.column.backref.is_some()
}

pub(super) fn is_write_candidate(field: &FieldMeta) -> bool {
    !is_skip(field) && !is_reference(field) && !field.column.read_only && !field.column.skip_bind
}

/// Returns whether a field participates in insert payloads.
pub(super) fn is_insertable(field: &FieldMeta) -> bool {
    is_write_candidate(field) && field.column.insertable.unwrap_or(true)
}

/// Returns whether a field participates in update payloads.
pub(super) fn is_updateable(field: &FieldMeta, primary_keys: &[String]) -> bool {
    is_write_candidate(field)
        && field.column.updatable.unwrap_or(true)
        && !is_primary_key(field, primary_keys)
}

pub(super) fn is_primary_key(field: &FieldMeta, primary_keys: &[String]) -> bool {
    let name = field
        .column
        .name
        .as_ref()
        .map(|name| name.value())
        .or_else(|| field.ident.as_ref().map(ToString::to_string));
    name.is_some_and(|name| primary_keys.iter().any(|key| key == &name))
}

/// Returns whether the type is a recognized canonical `Option<T>`.
pub(super) fn is_option(ty: &Type) -> bool {
    option_inner_type(ty).is_some()
}

/// Returns the inner type for canonical `Option<T>` spellings.
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

/// Returns the element type for canonical `Vec<T>` and `Option<Vec<T>>` spellings.
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
    canonical_path(path, "Option", "option")
}

fn is_canonical_vec(path: &TypePath) -> bool {
    canonical_path(path, "Vec", "vec")
}

/// Recognizes unqualified and standard-library qualified container paths.
fn canonical_path(path: &TypePath, type_name: &str, module: &str) -> bool {
    if path.qself.is_some() {
        return false;
    }
    let segments = path
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>();
    matches!(segments.as_slice(), [name] if name == type_name)
        || matches!(segments.as_slice(), [root, middle, name]
            if (root == "std" || root == "core" || root == "alloc")
                && middle == module
                && name == type_name)
}

/// Returns whether a field uses JSON serialization.
pub(super) fn is_json(field: &FieldMeta) -> bool {
    field.column.json
        || field
            .column
            .sql_type
            .as_ref()
            .map(|ty| matches!(ty.value().to_ascii_lowercase().as_str(), "json" | "jsonb"))
            .unwrap_or(false)
}

/// Returns whether a field participates in projections.
pub(super) fn is_selectable(field: &FieldMeta) -> bool {
    field.column.selectable.unwrap_or(true)
}
