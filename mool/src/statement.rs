use std::str::FromStr;
use std::sync::Arc;

use crate::QueryError;
use crate::commons::{Arguments, Database};

/// SQL text and database arguments ready for execution.
#[derive(Clone, Debug)]
pub struct Statement {
    pub(crate) sql: String,
    pub(crate) args: Arguments<'static>,
    pub(crate) error: Option<Arc<sqlx::error::BoxDynError>>,
}

impl Statement {
    /// Builds a statement from SQL text and already prepared arguments.
    pub fn new(sql: &str, args: Arguments<'static>) -> Self {
        Self {
            sql: sql.to_string(),
            args,
            error: None,
        }
    }

    /// Appends one positional bind value to this statement.
    pub fn bind<T>(mut self, val: T) -> Self
    where
        T: for<'q> sqlx::Encode<'q, Database> + sqlx::Type<Database> + Send + 'static,
    {
        use sqlx::Arguments as _;
        match self.args.add(val) {
            Ok(()) => self,
            Err(e) => {
                self.error = Some(Arc::new(e));
                self
            }
        }
    }

    /// Builds a statement with no arguments.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(sql: &str) -> Self {
        Self {
            sql: sql.to_string(),
            args: Arguments::default(),
            error: None,
        }
    }

    /// Returns the SQL text that will be executed.
    pub fn sql(&self) -> &str {
        &self.sql
    }

    /// Returns the prepared SQLx arguments.
    pub fn arguments(&self) -> &Arguments<'static> {
        &self.args
    }

    /// Returns the SQL and arguments, or a bind error if one occurred.
    pub fn into_parts(self) -> Result<(String, Arguments<'static>), QueryError> {
        if let Some(err) = self.error {
            return Err(QueryError::BindError(err.to_string()));
        }
        Ok((self.sql, self.args))
    }
}

impl FromStr for Statement {
    type Err = std::convert::Infallible;

    fn from_str(sql: &str) -> Result<Self, Self::Err> {
        Ok(Self {
            sql: sql.to_string(),
            args: Arguments::default(),
            error: None,
        })
    }
}
