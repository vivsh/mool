//! Request-parsed typed ordering for query scopes.

use std::borrow::Cow;
use std::str::FromStr;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};

use crate::interfaces::Model;
use crate::queries::__private::HasCols;

use super::{SortBuilder, Sortable};

/// Direction applied to one request sort key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortDirection {
    /// Ascending SQL order.
    Asc,
    /// Descending SQL order.
    Desc,
}

/// Finite vocabulary that resolves request sort keys to typed model columns.
///
/// [`derive@crate::SortKey`] generates model-bound implementations. Manual
/// implementations are supported for application-owned vocabularies and must
/// provide a finite [`SortKey::MAX_TERMS`] cap.
pub trait SortKey: Sized + 'static {
    /// The model whose typed columns this vocabulary may order.
    type Model: Model + HasCols;

    /// Stable name used when generating the request-sort schema.
    const NAME: &'static str;

    /// Maximum accepted comma-separated terms for this vocabulary.
    const MAX_TERMS: usize;

    /// Returns every accepted public request key.
    fn keys() -> &'static [&'static str];

    /// Resolves one public request key to its generated variant.
    fn parse_key(key: &str) -> Option<Self>;

    /// Returns the public key represented by this variant.
    fn key(&self) -> &'static str;

    /// Appends this key's typed order expression to `sort`.
    fn apply_sort(
        &self,
        direction: SortDirection,
        sort: SortBuilder<Self::Model>,
    ) -> SortBuilder<Self::Model>;
}

/// Parsed request ordering for one generated [`SortKey`] vocabulary.
pub struct Sort<K: SortKey> {
    terms: Vec<SortTerm<K>>,
}

struct SortTerm<K> {
    key: K,
    direction: SortDirection,
}

/// Validation error for one request-sort specification.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SortParseError {
    /// A comma-separated term was empty.
    #[error("sort term {position} is empty")]
    EmptyTerm { position: usize },
    /// A sort term contained ASCII whitespace.
    #[error("sort term {position} contains whitespace")]
    Whitespace { position: usize },
    /// A sort term used an unsupported direction prefix.
    #[error("invalid sort direction prefix in '{term}'")]
    InvalidPrefix { term: String },
    /// A requested key is outside the generated vocabulary.
    #[error("unknown sort key '{key}'")]
    UnknownKey { key: String },
    /// One key appeared more than once in the request.
    #[error("duplicate sort key '{key}'")]
    DuplicateKey { key: String },
    /// A request exceeded the vocabulary's configured term cap.
    #[error("sort accepts at most {max} terms")]
    TooManyTerms { max: usize },
}

impl<K: SortKey> Sort<K> {
    /// Parses one comma-separated request-sort value.
    pub fn parse(value: &str) -> Result<Self, SortParseError> {
        let count = value.split(',').count();
        if count > K::MAX_TERMS {
            return Err(SortParseError::TooManyTerms { max: K::MAX_TERMS });
        }
        let mut terms = Vec::with_capacity(count);
        for (index, term) in value.split(',').enumerate() {
            let parsed = parse_term::<K>(term, index + 1)?;
            if terms.iter().any(|existing: &SortTerm<K>| existing.key.key() == parsed.key.key()) {
                return Err(SortParseError::DuplicateKey {
                    key: parsed.key.key().to_owned(),
                });
            }
            terms.push(parsed);
        }
        Ok(Self { terms })
    }

    /// Returns whether the request omitted sorting.
    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }

    /// Returns the number of accepted request-sort terms.
    pub fn len(&self) -> usize {
        self.terms.len()
    }
}

impl<K: SortKey> Default for Sort<K> {
    fn default() -> Self {
        Self { terms: Vec::new() }
    }
}

impl<K: SortKey> FromStr for Sort<K> {
    type Err = SortParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl<K: SortKey> Sortable for Sort<K> {
    type Model = K::Model;

    fn apply_sort(&self, mut sort: SortBuilder<Self::Model>) -> SortBuilder<Self::Model> {
        for term in &self.terms {
            sort = term.key.apply_sort(term.direction, sort);
        }
        sort
    }
}

impl<'de, K: SortKey> serde::Deserialize<'de> for Sort<K> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(serde::de::Error::custom)
    }
}

impl<K: SortKey> JsonSchema for Sort<K> {
    fn inline_schema() -> bool {
        true
    }

    fn schema_name() -> Cow<'static, str> {
        format!("Sort{}", K::NAME).into()
    }

    fn schema_id() -> Cow<'static, str> {
        format!("mool::sorting::Sort<{}>", K::NAME).into()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        let keys = K::keys().join(", ");
        let example = K::keys()
            .first()
            .map_or_else(String::new, |key| format!("-{key}"));
        let description = format!(
            "Comma-separated sort keys: {keys}. Prefix a key with '-' for descending order. At most {} terms are accepted; duplicate keys are rejected.",
            K::MAX_TERMS
        );
        json_schema!({
            "type": "string",
            "description": description,
            "examples": [example]
        })
    }
}

fn parse_term<K: SortKey>(term: &str, position: usize) -> Result<SortTerm<K>, SortParseError> {
    if term.is_empty() {
        return Err(SortParseError::EmptyTerm { position });
    }
    if term.bytes().any(|byte| byte.is_ascii_whitespace()) {
        return Err(SortParseError::Whitespace { position });
    }
    let (direction, key) = if let Some(key) = term.strip_prefix('-') {
        if key.is_empty() || key.starts_with('-') || key.starts_with('+') {
            return Err(SortParseError::InvalidPrefix {
                term: term.to_owned(),
            });
        }
        (SortDirection::Desc, key)
    } else {
        if term.starts_with('+') {
            return Err(SortParseError::InvalidPrefix {
                term: term.to_owned(),
            });
        }
        (SortDirection::Asc, term)
    };
    let key = K::parse_key(key).ok_or_else(|| SortParseError::UnknownKey {
        key: key.to_owned(),
    })?;
    Ok(SortTerm { key, direction })
}
