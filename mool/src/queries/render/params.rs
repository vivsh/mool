//! Parameter and placeholder planning for typed-query rendering.

use std::collections::HashMap;
use std::sync::Arc;

use indexmap::IndexMap;

use crate::argvalue::ArgValue;
use crate::placeholders::Dialect;

use super::super::GENERATED_PREFIX;
use super::super::expr::ValueNode;
use super::super::handles::VarId;
use super::super::plan::{ParamSource, ParamSpec, QueryPlan};
use super::super::validate::{validate_param_compatible, validate_var_name};
use super::{PlannedParam, Renderer};
use crate::QueryError;

impl Renderer {
    pub(in crate::queries) fn plan(
        self,
        sql: String,
        result_type: Option<&'static str>,
        var_values: HashMap<VarId, ArgValue>,
    ) -> QueryPlan {
        let mut params = IndexMap::new();
        let mut values = HashMap::new();
        for (key, planned) in self.params {
            if let Some(value) = planned.value {
                values.insert(key.clone(), value);
            }
            params.insert(key, planned.spec);
        }
        for (id, value) in var_values {
            let name = match self.var_names.get(&id) {
                Some(name) => name.clone(),
                None => format!("__unused_var_{}", id.value()),
            };
            values.insert(name, value);
        }
        let dynamic_bind_count = self.bind_order.len();
        let total_bind_count = self.prebound_count + dynamic_bind_count;
        QueryPlan {
            sql,
            params,
            result_type,
            prebound_count: self.prebound_count,
            dynamic_bind_count,
            total_bind_count,
            values,
            bind_order: self.bind_order,
        }
    }

    pub(super) fn render_value(&mut self, value: &ValueNode) -> Result<String, QueryError> {
        match value {
            ValueNode::Val {
                name,
                rust_type,
                value,
            } => {
                let final_name = match name {
                    Some(name) => name.clone(),
                    None => self.next_value_name(),
                };
                let position = self.push_param(
                    final_name,
                    Some(*rust_type),
                    ParamSource::Val,
                    Some(value.clone()),
                    None,
                    None,
                )?;
                Ok(self.placeholder(position))
            }
            ValueNode::Var {
                id,
                name,
                rust_type,
            } => {
                let name = self.var_name(*id, name.as_ref())?;
                let display_name = name.clone();
                let position = self.push_param(
                    name,
                    Some(*rust_type),
                    ParamSource::Var,
                    None,
                    Some(*id),
                    Some(display_name),
                )?;
                Ok(self.placeholder(position))
            }
        }
    }

    fn var_name(&mut self, id: VarId, name: Option<&Arc<str>>) -> Result<String, QueryError> {
        if let Some(existing) = self.var_names.get(&id) {
            return Ok(existing.clone());
        }
        let value = match name {
            Some(name) => self.named_var(name.as_ref())?,
            None => self.next_var_name(),
        };
        self.var_names.insert(id, value.clone());
        Ok(value)
    }

    fn named_var(&mut self, name: &str) -> Result<String, QueryError> {
        validate_var_name(name)?;
        if !self.named_vars.insert(name.to_string()) {
            return Err(QueryError::BindError(format!(
                "duplicate placeholder name '{}'",
                name
            )));
        }
        Ok(name.to_string())
    }

    pub(super) fn placeholder(&self, position: usize) -> String {
        self.dialect_renderer.placeholder(position)
    }

    fn push_param(
        &mut self,
        name: String,
        rust_type: Option<&'static str>,
        source: ParamSource,
        value: Option<ArgValue>,
        var_id: Option<VarId>,
        display_name: Option<String>,
    ) -> Result<usize, QueryError> {
        match self.dialect {
            Dialect::Postgres => {
                self.push_postgres_param(name, rust_type, source, value, var_id, display_name)
            }
            Dialect::Mysql | Dialect::Sqlite => {
                self.push_positional_param(name, rust_type, source, value, var_id, display_name)
            }
        }
    }

    fn push_postgres_param(
        &mut self,
        name: String,
        rust_type: Option<&'static str>,
        source: ParamSource,
        value: Option<ArgValue>,
        var_id: Option<VarId>,
        display_name: Option<String>,
    ) -> Result<usize, QueryError> {
        if let Some(existing) = self.params.get_mut(&name) {
            validate_param_compatible(&name, &existing.spec, rust_type, source)?;
            existing.spec.occurrences.push(existing.spec.position);
            return Ok(existing.spec.position);
        }
        let position = self.prebound_count + self.params.len() + 1;
        self.bind_order.push(name.clone());
        self.params.insert(
            name.clone(),
            PlannedParam {
                spec: ParamSpec {
                    var_id,
                    display_name,
                    name,
                    position,
                    occurrences: vec![position],
                    rust_type,
                    sql_type: None,
                    source,
                },
                value,
            },
        );
        Ok(position)
    }

    fn push_positional_param(
        &mut self,
        name: String,
        rust_type: Option<&'static str>,
        source: ParamSource,
        value: Option<ArgValue>,
        var_id: Option<VarId>,
        display_name: Option<String>,
    ) -> Result<usize, QueryError> {
        let position = self.prebound_count + self.bind_order.len() + 1;
        self.bind_order.push(name.clone());
        if let Some(existing) = self.params.get_mut(&name) {
            validate_param_compatible(&name, &existing.spec, rust_type, source)?;
            existing.spec.occurrences.push(position);
            return Ok(position);
        }
        self.params.insert(
            name.clone(),
            PlannedParam {
                spec: ParamSpec {
                    var_id,
                    display_name,
                    name,
                    position,
                    occurrences: vec![position],
                    rust_type,
                    sql_type: None,
                    source,
                },
                value,
            },
        );
        Ok(position)
    }

    fn next_value_name(&mut self) -> String {
        self.value_counter += 1;
        format!("{GENERATED_PREFIX}{}", self.value_counter)
    }

    fn next_var_name(&mut self) -> String {
        self.var_counter += 1;
        format!("__var_{}", self.var_counter)
    }
}
