//! Dialect feature flags.

/// Feature gates that differ between SQL dialects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::queries) enum DialectFeature {
    Returning,
    Ilike,
    Upsert,
    WindowFunctions,
}

impl DialectFeature {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Returning => "RETURNING",
            Self::Ilike => "ILIKE",
            Self::Upsert => "upsert",
            Self::WindowFunctions => "window functions",
        }
    }
}
