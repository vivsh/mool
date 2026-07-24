use std::path::{Path, PathBuf};

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{LitStr, parse_macro_input};

pub(crate) fn embedded_migrations(
    input: TokenStream,
    runtime_path: proc_macro2::TokenStream,
) -> TokenStream {
    let path_lit = parse_macro_input!(input as LitStr);
    match expand_embedded_migrations(&path_lit, runtime_path) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.into_compile_error().into(),
    }
}

fn expand_embedded_migrations(
    path_lit: &LitStr,
    runtime_path: proc_macro2::TokenStream,
) -> syn::Result<proc_macro2::TokenStream> {
    let rel_path = path_lit.value();
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").map_err(|error| {
        syn::Error::new(
            path_lit.span(),
            format!("CARGO_MANIFEST_DIR is unavailable: {error}"),
        )
    })?;
    let dir = Path::new(&manifest_dir).join(&rel_path);
    let embedded_dir = std::fs::canonicalize(&dir).map_err(|error| {
        syn::Error::new(
            path_lit.span(),
            format!(
                "cannot read migration directory '{}': {error}",
                dir.display()
            ),
        )
    })?;
    if !embedded_dir.is_dir() {
        return Err(syn::Error::new(
            path_lit.span(),
            format!("migration path '{}' is not a directory", dir.display()),
        ));
    }
    let mut entries = migration_files(&embedded_dir, path_lit.span())?;
    entries.sort();
    let pairs = migration_pairs(&entries, path_lit.span())?;

    let dir_lit = LitStr::new(&embedded_dir.to_string_lossy(), Span::call_site());

    Ok(quote! {
        #runtime_path::migrations::EmbeddedMigrations {
            files: &[#(#pairs),*],
            dir: #dir_lit,
            children: &[],
        }
    })
}

fn migration_files(dir: &Path, span: Span) -> syn::Result<Vec<PathBuf>> {
    let entries = std::fs::read_dir(dir).map_err(|error| {
        syn::Error::new(
            span,
            format!(
                "cannot read migration directory '{}': {error}",
                dir.display()
            ),
        )
    })?;
    let mut files = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| {
            syn::Error::new(
                span,
                format!("cannot read migration directory entry: {error}"),
            )
        })?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("yaml") {
            files.push(path);
        }
    }
    Ok(files)
}

fn migration_pairs(entries: &[PathBuf], span: Span) -> syn::Result<Vec<proc_macro2::TokenStream>> {
    entries
        .iter()
        .map(|path| {
            let id = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .ok_or_else(|| {
                    syn::Error::new(
                        span,
                        format!("migration filename is not UTF-8: {}", path.display()),
                    )
                })?;
            let absolute = path.to_str().ok_or_else(|| {
                syn::Error::new(
                    span,
                    format!("migration path is not UTF-8: {}", path.display()),
                )
            })?;
            let id = LitStr::new(id, Span::call_site());
            let path = LitStr::new(absolute, Span::call_site());
            Ok(quote! { (#id, ::core::include_str!(#path)) })
        })
        .collect()
}
