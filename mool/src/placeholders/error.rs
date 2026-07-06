use std::sync::Arc;

/// Errors that can occur during placeholder resolution
#[derive(Debug, Clone)]
pub enum PlaceholderError {
    /// Placeholder not found in values map
    MissingValue(String),
    /// Failed to bind value to arguments
    BindError {
        placeholder: String,
        source: Arc<dyn std::error::Error + Send + Sync>,
    },
}

impl std::fmt::Display for PlaceholderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingValue(name) => write!(f, "placeholder '{}' not found in values map", name),
            Self::BindError {
                placeholder,
                source,
            } => {
                write!(
                    f,
                    "failed to bind placeholder '{}': {}",
                    placeholder, source
                )
            }
        }
    }
}

impl std::error::Error for PlaceholderError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::BindError { source, .. } => Some(source.as_ref()),
            _ => None,
        }
    }
}
