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
    let rel_path = path_lit.value();

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let dir = Path::new(&manifest_dir).join(&rel_path);
    let embedded_dir = absolute_dir(&dir);

    let mut entries = migration_files(&dir);
    entries.sort();

    let pairs = entries
        .iter()
        .map(|path| {
            let id = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or_default()
                .to_string();
            let abs = path.to_str().unwrap_or_default().to_string();
            let id_lit = LitStr::new(&id, Span::call_site());
            let path_lit = LitStr::new(&abs, Span::call_site());
            quote! { (#id_lit, ::core::include_str!(#path_lit)) }
        })
        .collect::<Vec<_>>();

    let dir_lit = LitStr::new(&embedded_dir.to_string_lossy(), Span::call_site());

    quote! {
        #runtime_path::EmbeddedMigrations {
            files: &[#(#pairs),*],
            dir: #dir_lit,
            children: &[],
        }
    }
    .into()
}

fn migration_files(dir: &Path) -> Vec<PathBuf> {
    if !dir.exists() {
        return Vec::new();
    }
    std::fs::read_dir(dir)
        .unwrap_or_else(|err| panic!("failed to read migrations dir '{}': {err}", dir.display()))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("yaml"))
        .collect()
}

fn absolute_dir(dir: &Path) -> PathBuf {
    std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf())
}
