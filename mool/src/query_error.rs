/// Error raised while planning or rendering a database query.
#[derive(Clone, Debug, thiserror::Error)]
pub enum QueryError {
    #[error("bind error: {0}")]
    BindError(String),
    #[error("source not set")]
    SourceNotSet,
    #[error("table metadata not set for {0}")]
    TableNotSet(&'static str),
    #[error("query source table mismatch: expected {expected}, got {got}")]
    TableMismatch { expected: String, got: String },
    #[error("unknown query alias or logical prefix '{0}'")]
    UnknownAlias(String),
    #[error("invalid projection field '{0}'")]
    InvalidProjection(String),
    #[error("reference '{reference}' is missing {field}")]
    MissingReference {
        reference: &'static str,
        field: &'static str,
    },
    #[error("unsupported filter operator '{0}' for this value")]
    UnsupportedFilter(&'static str),
    #[error("placeholder error: {0}")]
    PlaceholderError(#[from] crate::placeholders::PlaceholderError),
    #[error("missing binding for {0}")]
    MissingBinding(String),
    #[error("unused binding: {0}")]
    UnusedBinding(String),
    #[error("bind count mismatch: expected {expected}, got {got}")]
    BindCountMismatch { expected: usize, got: usize },
    #[error(
        "invalid identifier '{0}': start with an ASCII letter or underscore; use only ASCII alphanumerics and underscores"
    )]
    InvalidIdentifier(String),
}

/// Row locking mode for SELECT ... FOR UPDATE / FOR SHARE.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LockMode {
    Update,
    Share,
}
