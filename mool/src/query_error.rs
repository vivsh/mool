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
    #[error("invalid row lock: {reason}")]
    InvalidLock { reason: &'static str },
    #[error("query modifier '{modifier}' is not valid for {terminal}")]
    InvalidModifier {
        modifier: &'static str,
        terminal: &'static str,
    },
    #[error("batch size must be greater than zero, got {0}")]
    InvalidBatchSize(usize),
    #[error("{operation} requires at least one row")]
    EmptyBatch { operation: &'static str },
    #[error("batch parameter count overflow for {rows} rows and {columns} columns")]
    BatchParameterOverflow { rows: usize, columns: usize },
    #[error(
        "{operation} cannot fit {rows} rows in one statement: {required_parameters} parameters exceed the backend limit of {parameter_limit}"
    )]
    BatchTooLarge {
        operation: &'static str,
        rows: usize,
        columns: usize,
        required_parameters: usize,
        parameter_limit: usize,
    },
    #[error("batch operation requires {statements} SQL statements; use plans() to inspect them")]
    MultipleStatementsRequired { statements: usize },
    #[error("batch update contains duplicate primary key at rows {first} and {duplicate}")]
    DuplicateBatchKey { first: usize, duplicate: usize },
    #[error("batch column lengths differ: expected {expected}, got {got}")]
    MismatchedBatchColumns { expected: usize, got: usize },
}

/// Row locking mode for SELECT ... FOR UPDATE / FOR SHARE.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LockMode {
    /// Acquires an exclusive row lock suitable for updates.
    Update,
    /// Acquires a shared row lock.
    Share,
}

#[cfg(any(feature = "postgres", feature = "mysql"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LockWait {
    NoWait,
    SkipLocked,
}
