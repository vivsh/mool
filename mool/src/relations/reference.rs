//! Forward reference and join metadata shared by records and relations.

/// Supported SQL join kind for typed relation metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JoinType {
    /// `INNER JOIN`.
    Inner,
    /// `LEFT JOIN`.
    Left,
}

/// A single equality condition in a typed join.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JoinColumn {
    /// Column on the current/root side of the join.
    pub from: &'static str,
    /// Column on the joined table side of the join.
    pub to: &'static str,
}

/// Typed metadata for a SQL join target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReferenceMeta {
    /// Logical alias used for the joined table in generated SQL.
    pub logical_name: &'static str,
    /// Database table name for the joined side.
    pub table_name: &'static str,
    /// Optional database schema for the joined side.
    pub table_schema: Option<&'static str>,
    /// Equality join columns joined with `AND`.
    pub columns: &'static [JoinColumn],
    /// SQL join type.
    pub join_type: JoinType,
}
