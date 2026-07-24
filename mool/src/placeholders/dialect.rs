/// SQL dialect selected by the active backend feature.
///
/// Custom functions and expressions receive this value while rendering so one
/// implementation can provide portable behavior without compile-time dialect
/// generics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqlDialect {
    /// PostgreSQL uses $1, $2, $3, etc.
    Postgres,
    /// MySQL uses ? for all placeholders
    Mysql,
    /// MariaDB uses positional question-mark placeholders.
    Mariadb,
    /// SQLite uses ? for all placeholders
    Sqlite,
}

impl SqlDialect {
    pub(crate) const fn active() -> Self {
        #[cfg(feature = "postgres")]
        return Self::Postgres;

        #[cfg(feature = "mysql")]
        return Self::Mysql;

        #[cfg(feature = "mariadb")]
        return Self::Mariadb;

        #[cfg(feature = "sqlite")]
        return Self::Sqlite;
    }

    /// Returns the stable lowercase dialect name used in diagnostics.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Postgres => "postgres",
            Self::Mysql => "mysql",
            Self::Mariadb => "mariadb",
            Self::Sqlite => "sqlite",
        }
    }
}
