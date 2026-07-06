//! Dialect-aware SQL rendering, planning, and validation engine.
use indexmap::IndexMap;
use std::any::type_name;
use std::collections::{HashMap, HashSet};

use crate::argvalue::ArgValue;
use crate::interfaces::Record;
use crate::placeholders::Dialect;
use crate::relations::ReferenceMeta;

use super::dialect::{self, DialectRenderer};
use super::handles::VarId;
use super::plan::ParamSpec;
use super::source::Source;
use super::validate::{
    singular_alias, source_alias, source_key, validate_identifier, validate_reference,
    validate_source_shape,
};
use crate::QueryError;

mod expr;
mod params;
mod projection;
mod sources;
mod statements;
mod write;

#[derive(Clone)]
pub(super) struct SelectModel {
    pub(super) source: Source,
    pub(super) root_alias: String,
    pub(super) scan_root_alias: String,
    pub(super) references: IndexMap<String, ReferenceMeta>,
    pub(super) columns: Vec<String>,
    pub(super) result_type: &'static str,
}

#[derive(Clone, Copy)]
pub(super) enum RenderMode<'a> {
    Select(&'a SelectModel),
    MutationRoot { source: &'a Source },
}

impl SelectModel {
    pub(super) fn new<T>(source: &Source) -> Result<Self, QueryError>
    where
        T: Record,
    {
        validate_source_shape::<T>(source)?;
        let schema = T::record_schema();
        let scan_root_alias = match schema.root_name {
            Some(root_name) => root_name.to_string(),
            None => singular_alias(schema.table_name),
        };
        validate_identifier(&scan_root_alias)?;
        let root_alias = source_alias(source, &scan_root_alias);
        validate_identifier(&root_alias)?;
        let references = schema
            .references
            .into_iter()
            .map(|reference| {
                validate_reference(&reference)?;
                Ok((reference.logical_name.to_string(), reference))
            })
            .collect::<Result<IndexMap<_, _>, QueryError>>()?;
        Ok(Self {
            source: source.clone(),
            root_alias,
            scan_root_alias,
            references,
            columns: schema.column_names,
            result_type: type_name::<T>(),
        })
    }

    /// Builds a projection-free model over a source for aggregate terminals
    /// (`count`, `scalar`, `exists`). Only root-table columns are addressable;
    /// joined references require a row-shaped `Record` projection instead.
    pub(super) fn source_only(source: &Source) -> Result<Self, QueryError> {
        let scan_root_alias = singular_alias(source_key(source).2);
        validate_identifier(&scan_root_alias)?;
        let root_alias = source_alias(source, &scan_root_alias);
        validate_identifier(&root_alias)?;
        Ok(Self {
            source: source.clone(),
            root_alias,
            scan_root_alias,
            references: IndexMap::new(),
            columns: Vec::new(),
            result_type: "",
        })
    }
}

pub(super) struct Renderer {
    dialect: Dialect,
    dialect_renderer: &'static dyn DialectRenderer,
    params: IndexMap<String, PlannedParam>,
    bind_order: Vec<String>,
    var_names: HashMap<VarId, String>,
    named_vars: HashSet<String>,
    var_counter: usize,
    value_counter: usize,
    prebound_count: usize,
}

pub(super) struct PlannedParam {
    spec: ParamSpec,
    value: Option<ArgValue>,
}

impl Renderer {
    pub(super) fn new(dialect: Dialect) -> Self {
        Self {
            dialect,
            dialect_renderer: dialect::renderer(dialect),
            params: IndexMap::new(),
            bind_order: Vec::new(),
            var_names: HashMap::new(),
            named_vars: HashSet::new(),
            var_counter: 0,
            value_counter: 0,
            prebound_count: 0,
        }
    }

    pub(super) fn with_prebound(dialect: Dialect, prebound_count: usize) -> Self {
        Self {
            prebound_count,
            ..Self::new(dialect)
        }
    }

    pub(super) fn dialect(&self) -> Dialect {
        self.dialect
    }
}
