//! Reverse relation traits built from forward reference metadata.

use super::reference::ReferenceMeta;

/// Cardinality of a reverse relation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelationCardinality {
    /// At most one related row.
    One,
    /// Zero or more related rows.
    Many,
}

/// Reverse relation metadata between two table-backed models.
pub trait Backref: 'static {
    /// Parent/root model.
    type From: crate::Model;
    /// Related model reached through this backref.
    type To: crate::Model;

    /// Stable relation name used in diagnostics.
    const NAME: &'static str;
    /// Declared relation cardinality.
    const CARDINALITY: RelationCardinality;

    /// Reverse join metadata from `From` to `To`.
    fn meta() -> ReferenceMeta;
}

/// Marker for one-row reverse relations.
pub trait OneBackref: Backref {}

/// Marker for many-row reverse relations.
pub trait ManyBackref: Backref {}

/// Many-to-many relation represented by two typed joins.
pub trait ManyToMany: 'static {
    /// Parent/root model.
    type From: crate::Model;
    /// Join table model.
    type Through: crate::Model;
    /// Related model reached through the join table.
    type To: crate::Model;

    /// Stable relation name used in diagnostics.
    const NAME: &'static str;

    /// Join from parent model to through table.
    fn from_through() -> ReferenceMeta;

    /// Join from through table to related model.
    fn through_to() -> ReferenceMeta;
}
