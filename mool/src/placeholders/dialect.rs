/// Database dialect for placeholder formatting.
#[allow(dead_code)] // Non-selected variants are exercised by parser unit tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    /// PostgreSQL uses $1, $2, $3, etc.
    Postgres,
    /// MySQL uses ? for all placeholders
    Mysql,
    /// MariaDB uses positional question-mark placeholders.
    Mariadb,
    /// SQLite uses ? for all placeholders
    Sqlite,
}

impl Dialect {
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
}
