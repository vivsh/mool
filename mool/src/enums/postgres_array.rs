//! Backend-selected PostgreSQL array metadata for derived SQL enums.

#[cfg(feature = "postgres")]
#[doc(hidden)]
#[macro_export]
macro_rules! __mool_impl_sql_enum_pg_array {
    ($enum:ty, text) => {
        impl $crate::sqlx::postgres::PgHasArrayType for $enum {
            fn array_type_info() -> $crate::sqlx::postgres::PgTypeInfo {
                <String as $crate::sqlx::postgres::PgHasArrayType>::array_type_info()
            }
        }
    };
    ($enum:ty, int, $repr:ty) => {
        impl $crate::sqlx::postgres::PgHasArrayType for $enum {
            fn array_type_info() -> $crate::sqlx::postgres::PgTypeInfo {
                <$repr as $crate::sqlx::postgres::PgHasArrayType>::array_type_info()
            }
        }
    };
    ($enum:ty, native, $array_name:literal) => {
        impl $crate::sqlx::postgres::PgHasArrayType for $enum {
            fn array_type_info() -> $crate::sqlx::postgres::PgTypeInfo {
                $crate::sqlx::postgres::PgTypeInfo::with_name($array_name)
            }
        }
    };
}

#[cfg(not(feature = "postgres"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __mool_impl_sql_enum_pg_array {
    ($($tokens:tt)*) => {};
}
