//! Database expression marker types.

#[cfg(feature = "postgres")]
use std::marker::PhantomData;

/// Marker for SQL JSON/JSONB expressions in typed queries.
///
/// This is not a Rust value wrapper. Model fields keep their actual Rust type,
/// while generated query columns use this marker when the column is stored as
/// JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Json;

/// Marker for SQL array expressions in typed queries.
///
/// This is not a Rust value wrapper. Model fields keep using `Vec<T>` or
/// `Option<Vec<T>>`, while generated query columns use this marker for SQL
/// array expressions.
#[cfg(feature = "postgres")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Array<T>(PhantomData<fn() -> T>);

/// Dynamically sized single-precision vector value for similarity search.
#[cfg(feature = "vector")]
#[derive(Clone, Debug, PartialEq)]
pub struct Vector(pgvector::Vector);

/// Error raised when a vector value is malformed or violates a schema dimension.
#[cfg(feature = "vector")]
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum VectorError {
    /// The supplied or decoded vector length differs from the declared dimension.
    #[error("vector dimension mismatch: expected {expected}, got {actual}")]
    Dimension { expected: usize, actual: usize },
    /// A vector must contain at least one element.
    #[error("vector must contain at least one element")]
    Empty,
    /// Every vector element must be finite.
    #[error("vector element at index {index} is not finite")]
    NonFinite { index: usize },
    /// A database value is not a complete sequence of little-endian `f32` values.
    #[error("invalid vector binary representation with {byte_length} bytes")]
    InvalidEncoding { byte_length: usize },
}

#[cfg(feature = "vector")]
impl Vector {
    /// Creates a dynamically sized vector from an array.
    pub fn from_array<const D: usize>(values: [f32; D]) -> Self {
        Self(pgvector::Vector::from(values.to_vec()))
    }

    /// Creates a non-empty vector containing only finite elements.
    pub fn try_from_vec(values: Vec<f32>) -> Result<Self, VectorError> {
        if values.is_empty() {
            return Err(VectorError::Empty);
        }
        if let Some(index) = values.iter().position(|value| !value.is_finite()) {
            return Err(VectorError::NonFinite { index });
        }
        Ok(Self(pgvector::Vector::from(values)))
    }

    /// Validates this value against a fixed database-column dimension.
    pub fn validate_dimensions(&self, expected: usize) -> Result<(), VectorError> {
        let actual = self.dimensions();
        if actual == expected {
            return Ok(());
        }
        Err(VectorError::Dimension { expected, actual })
    }

    /// Borrows the vector elements in dimension order.
    pub fn as_slice(&self) -> &[f32] {
        self.0.as_slice()
    }

    /// Returns the runtime number of vector dimensions.
    pub fn dimensions(&self) -> usize {
        self.as_slice().len()
    }

    /// Consumes the vector and returns its elements.
    pub fn into_vec(self) -> Vec<f32> {
        self.0.to_vec()
    }
}

#[cfg(feature = "vector")]
impl<const D: usize> From<[f32; D]> for Vector {
    fn from(values: [f32; D]) -> Self {
        Self::from_array(values)
    }
}

#[cfg(feature = "vector")]
impl TryFrom<Vec<f32>> for Vector {
    type Error = VectorError;

    fn try_from(values: Vec<f32>) -> Result<Self, Self::Error> {
        Self::try_from_vec(values)
    }
}

#[cfg(all(feature = "vector", feature = "postgres"))]
impl sqlx::Type<sqlx::Postgres> for Vector {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <pgvector::Vector as sqlx::Type<sqlx::Postgres>>::type_info()
    }

    fn compatible(info: &sqlx::postgres::PgTypeInfo) -> bool {
        <pgvector::Vector as sqlx::Type<sqlx::Postgres>>::compatible(info)
    }
}

#[cfg(all(feature = "vector", feature = "postgres"))]
impl sqlx::Encode<'_, sqlx::Postgres> for Vector {
    fn encode_by_ref(
        &self,
        buffer: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        <pgvector::Vector as sqlx::Encode<sqlx::Postgres>>::encode_by_ref(&self.0, buffer)
    }
}

#[cfg(all(feature = "vector", feature = "postgres"))]
impl<'r> sqlx::Decode<'r, sqlx::Postgres> for Vector {
    fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let value = <pgvector::Vector as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
        Self::try_from_vec(value.to_vec()).map_err(Into::into)
    }
}

#[cfg(all(feature = "vector", any(feature = "mysql", feature = "mariadb")))]
impl sqlx::Type<sqlx::MySql> for Vector {
    fn type_info() -> sqlx::mysql::MySqlTypeInfo {
        <Vec<u8> as sqlx::Type<sqlx::MySql>>::type_info()
    }

    fn compatible(info: &sqlx::mysql::MySqlTypeInfo) -> bool {
        <Vec<u8> as sqlx::Type<sqlx::MySql>>::compatible(info)
    }
}

#[cfg(all(feature = "vector", any(feature = "mysql", feature = "mariadb")))]
impl sqlx::Encode<'_, sqlx::MySql> for Vector {
    fn encode_by_ref(
        &self,
        buffer: &mut Vec<u8>,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        let bytes = vector_le_bytes(self.as_slice());
        <Vec<u8> as sqlx::Encode<sqlx::MySql>>::encode_by_ref(&bytes, buffer)
    }
}

#[cfg(all(feature = "vector", any(feature = "mysql", feature = "mariadb")))]
impl<'r> sqlx::Decode<'r, sqlx::MySql> for Vector {
    fn decode(value: sqlx::mysql::MySqlValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let bytes = <Vec<u8> as sqlx::Decode<sqlx::MySql>>::decode(value)?;
        vector_from_le_bytes(&bytes)
            .and_then(Self::try_from_vec)
            .map_err(Into::into)
    }
}

#[cfg(all(feature = "vector", feature = "sqlite"))]
impl sqlx::Type<sqlx::Sqlite> for Vector {
    fn type_info() -> sqlx::sqlite::SqliteTypeInfo {
        <Vec<u8> as sqlx::Type<sqlx::Sqlite>>::type_info()
    }

    fn compatible(info: &sqlx::sqlite::SqliteTypeInfo) -> bool {
        <Vec<u8> as sqlx::Type<sqlx::Sqlite>>::compatible(info)
    }
}

#[cfg(all(feature = "vector", feature = "sqlite"))]
impl<'q> sqlx::Encode<'q, sqlx::Sqlite> for Vector {
    fn encode_by_ref(
        &self,
        arguments: &mut <sqlx::Sqlite as sqlx::Database>::ArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        <Vec<u8> as sqlx::Encode<sqlx::Sqlite>>::encode(vector_le_bytes(self.as_slice()), arguments)
    }
}

#[cfg(all(feature = "vector", feature = "sqlite"))]
impl<'r> sqlx::Decode<'r, sqlx::Sqlite> for Vector {
    fn decode(value: sqlx::sqlite::SqliteValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let bytes = <Vec<u8> as sqlx::Decode<sqlx::Sqlite>>::decode(value)?;
        vector_from_le_bytes(&bytes)
            .and_then(Self::try_from_vec)
            .map_err(Into::into)
    }
}

#[cfg(all(
    feature = "vector",
    any(feature = "mysql", feature = "mariadb", feature = "sqlite")
))]
fn vector_le_bytes(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

#[cfg(all(
    feature = "vector",
    any(feature = "mysql", feature = "mariadb", feature = "sqlite")
))]
fn vector_from_le_bytes(bytes: &[u8]) -> Result<Vec<f32>, VectorError> {
    if !bytes.len().is_multiple_of(std::mem::size_of::<f32>()) {
        return Err(VectorError::InvalidEncoding {
            byte_length: bytes.len(),
        });
    }
    Ok(bytes
        .chunks_exact(std::mem::size_of::<f32>())
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect())
}
