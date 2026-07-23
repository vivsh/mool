//! Code generation for `SqlEnum`.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::LitInt;

use super::attrs::{IntRepr, Storage};
use super::model::ParsedSqlEnum;

pub fn generate(parsed: &ParsedSqlEnum, crate_path: &TokenStream) -> TokenStream {
    let ident = &parsed.ident;
    let sql_name = &parsed.sql_name;
    let labels = parsed
        .variants
        .iter()
        .map(|variant| variant.label.as_str())
        .collect::<Vec<_>>();
    let variant_idents = parsed
        .variants
        .iter()
        .map(|variant| &variant.ident)
        .collect::<Vec<_>>();
    let storage = storage_tokens(parsed.storage, crate_path);
    let common = common_impl(parsed, crate_path);
    let int_impl = int_impl(parsed, crate_path);
    let sqlx_impls = sqlx_impls(parsed, crate_path);
    let postgres_array = postgres_array_impl(parsed, crate_path);
    let column_type = column_type_impl(parsed, crate_path);

    quote! {
        impl #ident {
            pub const SQL_NAME: &'static str = #sql_name;
            pub const SQL_STORAGE: #crate_path::SqlEnumStorage = #storage;
            pub const SQL_VALUES: &'static [&'static str] = &[#(#labels),*];

            pub fn as_sql_str(self) -> &'static str {
                match self {
                    #(Self::#variant_idents => #labels,)*
                }
            }

            pub fn try_from_sql_str(value: &str) -> Result<Self, #crate_path::SqlEnumError> {
                match value {
                    #(#labels => Ok(Self::#variant_idents),)*
                    other => Err(#crate_path::SqlEnumError::UnknownLabel {
                        enum_name: Self::SQL_NAME,
                        value: other.to_string(),
                    }),
                }
            }
        }

        impl ::core::fmt::Display for #ident {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                f.write_str(self.as_sql_str())
            }
        }

        impl ::core::str::FromStr for #ident {
            type Err = #crate_path::SqlEnumError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::try_from_sql_str(value)
            }
        }

        #common
        #int_impl
        #sqlx_impls
        #postgres_array
        #column_type
    }
}

fn postgres_array_impl(parsed: &ParsedSqlEnum, crate_path: &TokenStream) -> TokenStream {
    let ident = &parsed.ident;
    match parsed.storage {
        Storage::Int => {
            let repr = repr_ty(parsed.repr);
            quote! { #crate_path::__mool_impl_sql_enum_pg_array!(#ident, int, #repr); }
        }
        Storage::NativePostgres => {
            let array_name = format!("_{}", parsed.sql_name);
            quote! { #crate_path::__mool_impl_sql_enum_pg_array!(#ident, native, #array_name); }
        }
        Storage::Text | Storage::NativeMysql => {
            quote! { #crate_path::__mool_impl_sql_enum_pg_array!(#ident, text); }
        }
    }
}

fn common_impl(parsed: &ParsedSqlEnum, crate_path: &TokenStream) -> TokenStream {
    let ident = &parsed.ident;
    let sql_column_type = sql_column_type_expr(parsed, crate_path);
    let check_expr = check_expr(parsed, crate_path);
    quote! {
        impl #crate_path::SqlEnum for #ident {
            const SQL_NAME: &'static str = Self::SQL_NAME;
            const SQL_STORAGE: #crate_path::SqlEnumStorage = Self::SQL_STORAGE;
            const SQL_VALUES: &'static [&'static str] = Self::SQL_VALUES;

            fn as_sql_str(self) -> &'static str {
                Self::as_sql_str(self)
            }

            fn try_from_sql_str(value: &str) -> Result<Self, #crate_path::SqlEnumError> {
                Self::try_from_sql_str(value)
            }

            fn sql_column_type(dialect: #crate_path::gaman::core::Dialect) -> String {
                #sql_column_type
            }

            fn sql_check_expr(column: &str, dialect: #crate_path::gaman::core::Dialect) -> Option<String> {
                #check_expr
            }
        }
    }
}

fn int_impl(parsed: &ParsedSqlEnum, crate_path: &TokenStream) -> TokenStream {
    if parsed.storage != Storage::Int {
        return TokenStream::new();
    }
    let ident = &parsed.ident;
    let repr = repr_ty(parsed.repr);
    let variant_idents = parsed
        .variants
        .iter()
        .map(|variant| &variant.ident)
        .collect::<Vec<_>>();
    let codes = parsed
        .variants
        .iter()
        .map(|variant| {
            typed_int_literal(variant.code.expect("validated int enum code"), parsed.repr)
        })
        .collect::<Vec<_>>();
    quote! {
        impl #ident {
            pub const SQL_CODES: &'static [#repr] = &[#(#codes as #repr),*];

            pub fn as_sql_code(self) -> #repr {
                match self {
                    #(Self::#variant_idents => #codes as #repr,)*
                }
            }

            pub fn try_from_sql_code(value: #repr) -> Result<Self, #crate_path::SqlEnumError> {
                match value {
                    #(#codes => Ok(Self::#variant_idents),)*
                    other => Err(#crate_path::SqlEnumError::UnknownCode {
                        enum_name: Self::SQL_NAME,
                        value: other as i64,
                    }),
                }
            }
        }
    }
}

fn sqlx_impls(parsed: &ParsedSqlEnum, crate_path: &TokenStream) -> TokenStream {
    generic_sqlx_impl(parsed, crate_path)
}

fn generic_sqlx_impl(parsed: &ParsedSqlEnum, crate_path: &TokenStream) -> TokenStream {
    let ident = &parsed.ident;
    let delegate = delegate_ty(parsed);
    let encode = generic_encode(parsed, crate_path);
    let decode = generic_decode(parsed, crate_path);
    quote! {
        impl<DB> #crate_path::sqlx::Type<DB> for #ident
        where
            DB: #crate_path::sqlx::Database,
            #delegate: #crate_path::sqlx::Type<DB>,
        {
            fn type_info() -> <DB as #crate_path::sqlx::Database>::TypeInfo {
                <#delegate as #crate_path::sqlx::Type<DB>>::type_info()
            }

            fn compatible(ty: &<DB as #crate_path::sqlx::Database>::TypeInfo) -> bool {
                <#delegate as #crate_path::sqlx::Type<DB>>::compatible(ty)
            }
        }

        impl<'q, DB> #crate_path::sqlx::Encode<'q, DB> for #ident
        where
            DB: #crate_path::sqlx::Database,
            #delegate: #crate_path::sqlx::Encode<'q, DB>,
        {
            fn encode_by_ref(
                &self,
                buf: &mut <DB as #crate_path::sqlx::Database>::ArgumentBuffer<'q>,
            ) -> Result<#crate_path::sqlx::encode::IsNull, #crate_path::sqlx::error::BoxDynError> {
                #encode
            }
        }

        impl<'r, DB> #crate_path::sqlx::Decode<'r, DB> for #ident
        where
            DB: #crate_path::sqlx::Database,
            #delegate: #crate_path::sqlx::Decode<'r, DB>,
        {
            fn decode(
                value: <DB as #crate_path::sqlx::Database>::ValueRef<'r>,
            ) -> Result<Self, #crate_path::sqlx::error::BoxDynError> {
                #decode
            }
        }
    }
}

fn generic_encode(parsed: &ParsedSqlEnum, crate_path: &TokenStream) -> TokenStream {
    if parsed.storage == Storage::Int {
        let ty = repr_ty(parsed.repr);
        return quote! {
            let value: #ty = self.as_sql_code();
            <#ty as #crate_path::sqlx::Encode<'q, DB>>::encode_by_ref(&value, buf)
        };
    }
    quote! {
        let value = self.as_sql_str().to_string();
        <String as #crate_path::sqlx::Encode<'q, DB>>::encode_by_ref(&value, buf)
    }
}

fn generic_decode(parsed: &ParsedSqlEnum, crate_path: &TokenStream) -> TokenStream {
    if parsed.storage == Storage::Int {
        let ty = repr_ty(parsed.repr);
        return quote! {
            let value = <#ty as #crate_path::sqlx::Decode<'r, DB>>::decode(value)?;
            Self::try_from_sql_code(value)
                .map_err(|err| -> #crate_path::sqlx::error::BoxDynError { Box::new(err) })
        };
    }
    quote! {
        let value = <String as #crate_path::sqlx::Decode<'r, DB>>::decode(value)?;
        Self::try_from_sql_str(&value)
            .map_err(|err| -> #crate_path::sqlx::error::BoxDynError { Box::new(err) })
    }
}

fn column_type_impl(parsed: &ParsedSqlEnum, crate_path: &TokenStream) -> TokenStream {
    let ident = &parsed.ident;
    let static_sql_type = static_column_type_expr(parsed, crate_path);
    quote! {
        impl #crate_path::schema::ColumnType for #ident {
            fn column_desc(dialect: &#crate_path::gaman::core::Dialect) -> #crate_path::schema::ColumnDesc {
                #crate_path::schema::ColumnDesc {
                    sql_type: #static_sql_type,
                    nullable: false,
                }
            }
        }
    }
}

fn static_column_type_expr(parsed: &ParsedSqlEnum, crate_path: &TokenStream) -> TokenStream {
    match parsed.storage {
        Storage::Text => quote! { "text" },
        Storage::Int => {
            let pg = int_sql_type_postgres(parsed.repr);
            quote! {
                match dialect {
                    #crate_path::gaman::core::Dialect::Postgres => #pg,
                    _ => "integer",
                }
            }
        }
        Storage::NativePostgres => quote! {
            match dialect {
                #crate_path::gaman::core::Dialect::Postgres => Self::SQL_NAME,
                _ => "text",
            }
        },
        Storage::NativeMysql => {
            let mysql_type = mysql_enum_type_literal(parsed);
            quote! { #mysql_type }
        }
    }
}

fn sql_column_type_expr(parsed: &ParsedSqlEnum, crate_path: &TokenStream) -> TokenStream {
    match parsed.storage {
        Storage::Text => quote! { "text".to_string() },
        Storage::Int => {
            let pg = int_sql_type_postgres(parsed.repr);
            quote! {
                match dialect {
                    #crate_path::gaman::core::Dialect::Postgres => #pg.to_string(),
                    _ => "integer".to_string(),
                }
            }
        }
        Storage::NativePostgres => quote! {
            match dialect {
                #crate_path::gaman::core::Dialect::Postgres => Self::SQL_NAME.to_string(),
                _ => "text".to_string(),
            }
        },
        Storage::NativeMysql => quote! {
            #crate_path::enums::__private::mysql_enum_type(Self::SQL_VALUES)
        },
    }
}

fn check_expr(parsed: &ParsedSqlEnum, crate_path: &TokenStream) -> TokenStream {
    match parsed.storage {
        Storage::Text => {
            quote! { Some(#crate_path::enums::__private::text_check_expr(column, Self::SQL_VALUES)) }
        }
        Storage::Int => {
            quote! { Some(#crate_path::enums::__private::int_check_expr(column, Self::SQL_CODES)) }
        }
        Storage::NativePostgres => quote! {
            match dialect {
                #crate_path::gaman::core::Dialect::Postgres => None,
                _ => Some(#crate_path::enums::__private::text_check_expr(column, Self::SQL_VALUES)),
            }
        },
        Storage::NativeMysql => quote! { None },
    }
}

fn storage_tokens(storage: Storage, crate_path: &TokenStream) -> TokenStream {
    match storage {
        Storage::Text => quote! { #crate_path::SqlEnumStorage::Text },
        Storage::Int => quote! { #crate_path::SqlEnumStorage::Int },
        Storage::NativePostgres => quote! { #crate_path::SqlEnumStorage::NativePostgres },
        Storage::NativeMysql => quote! { #crate_path::SqlEnumStorage::NativeMysql },
    }
}

fn delegate_ty(parsed: &ParsedSqlEnum) -> TokenStream {
    if parsed.storage == Storage::Int {
        return repr_ty(parsed.repr);
    }
    quote! { String }
}

fn repr_ty(repr: IntRepr) -> TokenStream {
    let ident = match repr {
        IntRepr::I16 => format_ident!("i16"),
        IntRepr::I32 => format_ident!("i32"),
        IntRepr::I64 => format_ident!("i64"),
    };
    quote! { #ident }
}

fn int_sql_type_postgres(repr: IntRepr) -> &'static str {
    match repr {
        IntRepr::I16 => "smallint",
        IntRepr::I32 => "integer",
        IntRepr::I64 => "bigint",
    }
}

fn typed_int_literal(value: i64, repr: IntRepr) -> LitInt {
    let suffix = match repr {
        IntRepr::I16 => "i16",
        IntRepr::I32 => "i32",
        IntRepr::I64 => "i64",
    };
    LitInt::new(&format!("{value}_{suffix}"), proc_macro2::Span::call_site())
}

fn mysql_enum_type_literal(parsed: &ParsedSqlEnum) -> String {
    let values = parsed
        .variants
        .iter()
        .map(|variant| format!("'{}'", variant.label.replace('\'', "''")))
        .collect::<Vec<_>>()
        .join(", ");
    format!("ENUM({values})")
}
