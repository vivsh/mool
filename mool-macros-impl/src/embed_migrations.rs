//! Function-like macro implementation for embedding migration YAML files.

use std::path::{Path, PathBuf};

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::LitStr;

/// Expands an `embed_migrations!` invocation for the resolved Mool runtime path.
pub fn expand(input: TokenStream, runtime_path: TokenStream) -> TokenStream {
    let path_lit = match syn::parse2::<LitStr>(input) {
        Ok(path_lit) => path_lit,
        Err(error) => return error.into_compile_error(),
    };
    match expand_path(&path_lit, runtime_path) {
        Ok(tokens) => tokens,
        Err(error) => error.into_compile_error(),
    }
}

/// Resolves, validates, and embeds one migration directory.
fn expand_path(path_lit: &LitStr, runtime_path: TokenStream) -> syn::Result<TokenStream> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").map_err(|error| {
        syn::Error::new(
            path_lit.span(),
            format!("CARGO_MANIFEST_DIR is unavailable: {error}"),
        )
    })?;
    let source_dir = Path::new(&manifest_dir).join(path_lit.value());
    let embedded_dir = std::fs::canonicalize(&source_dir).map_err(|error| {
        syn::Error::new(
            path_lit.span(),
            format!(
                "cannot read migration directory '{}': {error}",
                source_dir.display()
            ),
        )
    })?;
    if !embedded_dir.is_dir() {
        return Err(syn::Error::new(
            path_lit.span(),
            format!(
                "migration path '{}' is not a directory",
                source_dir.display()
            ),
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

/// Collects regular YAML files in one migration directory.
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
        if path.extension().and_then(|extension| extension.to_str()) != Some("yaml") {
            continue;
        }
        let file_type = entry.file_type().map_err(|error| {
            syn::Error::new(
                span,
                format!(
                    "cannot inspect migration entry '{}': {error}",
                    path.display()
                ),
            )
        })?;
        if !file_type.is_file() {
            return Err(syn::Error::new(
                span,
                format!("migration entry '{}' is not a file", path.display()),
            ));
        }
        files.push(path);
    }
    Ok(files)
}

/// Produces migration identifiers and `include_str!` expressions for sorted files.
fn migration_pairs(entries: &[PathBuf], span: Span) -> syn::Result<Vec<TokenStream>> {
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
