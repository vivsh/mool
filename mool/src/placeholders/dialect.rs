/// Database dialect for placeholder formatting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    /// PostgreSQL uses $1, $2, $3, etc.
    Postgres,
    /// MySQL uses ? for all placeholders
    Mysql,
    /// SQLite uses ? for all placeholders
    Sqlite,
}

impl Dialect {
    pub(crate) const fn active() -> Self {
        #[cfg(feature = "postgres")]
        return Self::Postgres;

        #[cfg(all(feature = "mysql", not(feature = "postgres")))]
        return Self::Mysql;

        #[cfg(all(feature = "sqlite", not(any(feature = "postgres", feature = "mysql"))))]
        return Self::Sqlite;

        // No DB feature active — dummy mode uses SQLite placeholder syntax.
        #[cfg(not(any(feature = "postgres", feature = "mysql", feature = "sqlite")))]
        return Self::Sqlite;
    }
}
