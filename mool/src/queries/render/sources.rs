//! Source, CTE, FROM, and implicit-reference rendering.

use crate::relations::{JoinType, ReferenceMeta};

use super::super::handles::Table;
use super::super::scope::QueryScope;
use super::super::source::{CteSource, Source, SourceColumnRef};
use super::super::validate::{validate_identifier, validate_source_column};
use super::{Renderer, SelectModel};
use crate::QueryError;

impl Renderer {
    pub(super) fn render_mutation_with(
        &mut self,
        scope: &QueryScope,
        sql: &mut String,
    ) -> Result<(), QueryError> {
        let inline = matches!(
            self.dialect(),
            crate::placeholders::Dialect::Mysql | crate::placeholders::Dialect::Mariadb
        );
        let ctes = scope.ctes.iter().filter(|cte| {
            !inline
                || !scope
                    .mutation_sources
                    .contains(&Source::Cte((*cte).clone()))
        });
        self.render_cte_list(ctes, sql)
    }

    pub(super) fn render_with(
        &mut self,
        scope: &QueryScope,
        sql: &mut String,
    ) -> Result<(), QueryError> {
        if scope.ctes.is_empty() {
            return Ok(());
        }
        sql.push_str("WITH ");
        for (idx, cte) in scope.ctes.iter().enumerate() {
            if idx > 0 {
                sql.push_str(", ");
            }
            self.render_cte(cte, sql)?;
        }
        sql.push(' ');
        Ok(())
    }

    fn render_cte_list<'a>(
        &mut self,
        ctes: impl Iterator<Item = &'a CteSource>,
        sql: &mut String,
    ) -> Result<(), QueryError> {
        let mut rendered = Vec::new();
        for cte in ctes {
            let mut item = String::new();
            self.render_cte(cte, &mut item)?;
            rendered.push(item);
        }
        if !rendered.is_empty() {
            sql.push_str("WITH ");
            sql.push_str(&rendered.join(", "));
            sql.push(' ');
        }
        Ok(())
    }

    pub(super) fn render_mutation_sources(
        &mut self,
        sources: &[Source],
        sql: &mut String,
    ) -> Result<(), QueryError> {
        for (index, source) in sources.iter().enumerate() {
            if index > 0 {
                sql.push_str(", ");
            }
            self.render_mutation_source(source, index, sql)?;
        }
        Ok(())
    }

    fn render_mutation_source(
        &mut self,
        source: &Source,
        index: usize,
        sql: &mut String,
    ) -> Result<(), QueryError> {
        let alias = super::mutation_source_alias(index);
        match source {
            Source::Table(table) => sql.push_str(&self.render_table_name(table)?),
            Source::Cte(cte) if self.inline_mutation_ctes() => {
                sql.push('(');
                sql.push_str(&self.render_select(
                    &cte.data.scope,
                    &cte.data.model,
                    cte.data.slice,
                )?);
                sql.push(')');
            }
            Source::Cte(cte) => sql.push_str(&cte.data.name),
            Source::Subquery(subquery) => {
                sql.push('(');
                sql.push_str(&self.render_select(
                    &subquery.data.scope,
                    &subquery.data.model,
                    subquery.data.slice,
                )?);
                sql.push(')');
            }
        }
        sql.push_str(" AS ");
        sql.push_str(&alias);
        Ok(())
    }

    fn inline_mutation_ctes(&self) -> bool {
        matches!(
            self.dialect(),
            crate::placeholders::Dialect::Mysql | crate::placeholders::Dialect::Mariadb
        )
    }

    pub(super) fn render_from(
        &mut self,
        model: &SelectModel,
        sql: &mut String,
    ) -> Result<(), QueryError> {
        sql.push_str(" FROM ");
        self.render_source(&model.source, &model.root_alias, sql)?;
        for reference in model.references.values() {
            self.render_reference(reference, model, sql)?;
        }
        Ok(())
    }

    pub(super) fn render_source_column_query(
        &mut self,
        column: &SourceColumnRef,
    ) -> Result<String, QueryError> {
        validate_source_column(column)?;
        match &column.source {
            Source::Cte(cte) => Ok(format!("SELECT {} FROM {}", column.name, cte.data.name)),
            Source::Subquery(subquery) => Ok(format!(
                "SELECT {}.{} FROM ({}) {}",
                subquery.data.name,
                column.name,
                self.render_select(
                    &subquery.data.scope,
                    &subquery.data.model,
                    subquery.data.slice
                )?,
                subquery.data.name
            )),
            Source::Table(table) => Ok(format!(
                "SELECT {} FROM {}",
                column.name,
                self.render_table_name(table)?
            )),
        }
    }

    pub(super) fn resolve_model_column(
        &self,
        column: &str,
        model: &SelectModel,
    ) -> Result<String, QueryError> {
        if let Some((owner, name)) = column.split_once('.') {
            return self.resolve_qualified(owner, name, model);
        }
        validate_identifier(column)?;
        Ok(format!("{}.{}", model.root_alias, column))
    }

    pub(super) fn render_table_name(&self, table: &Table) -> Result<String, QueryError> {
        validate_identifier(&table.data.name)?;
        if let Some(schema) = &table.data.schema {
            validate_identifier(schema)?;
            return Ok(format!("{schema}.{}", table.data.name));
        }
        Ok(table.data.name.to_string())
    }

    fn render_cte(&mut self, cte: &CteSource, sql: &mut String) -> Result<(), QueryError> {
        validate_identifier(&cte.data.name)?;
        sql.push_str(&cte.data.name);
        sql.push_str(" AS (");
        sql.push_str(&self.render_select(&cte.data.scope, &cte.data.model, cte.data.slice)?);
        sql.push(')');
        Ok(())
    }

    fn render_source(
        &mut self,
        source: &Source,
        alias: &str,
        sql: &mut String,
    ) -> Result<(), QueryError> {
        match source {
            Source::Table(table) => {
                sql.push_str(&self.render_table_name(table)?);
                if alias != table.data.name.as_ref() {
                    sql.push(' ');
                    sql.push_str(alias);
                }
            }
            Source::Cte(cte) => sql.push_str(&cte.data.name),
            Source::Subquery(subquery) => {
                sql.push('(');
                sql.push_str(&self.render_select(
                    &subquery.data.scope,
                    &subquery.data.model,
                    subquery.data.slice,
                )?);
                sql.push_str(") ");
                sql.push_str(alias);
            }
        }
        Ok(())
    }

    fn render_reference(
        &self,
        reference: &ReferenceMeta,
        model: &SelectModel,
        sql: &mut String,
    ) -> Result<(), QueryError> {
        let join = match reference.join_type {
            JoinType::Inner => " JOIN ",
            JoinType::Left => " LEFT JOIN ",
        };
        sql.push_str(join);
        self.render_reference_table(reference, sql)?;
        sql.push_str(" ON ");
        for (idx, column) in reference.columns.iter().enumerate() {
            if idx > 0 {
                sql.push_str(" AND ");
            }
            sql.push_str(reference.logical_name);
            sql.push('.');
            sql.push_str(column.to);
            sql.push_str(" = ");
            sql.push_str(&self.resolve_model_column(column.from, model)?);
        }
        Ok(())
    }

    fn render_reference_table(
        &self,
        reference: &ReferenceMeta,
        sql: &mut String,
    ) -> Result<(), QueryError> {
        if let Some(schema) = reference.table_schema {
            validate_identifier(schema)?;
            sql.push_str(schema);
            sql.push('.');
        }
        sql.push_str(reference.table_name);
        sql.push(' ');
        sql.push_str(reference.logical_name);
        Ok(())
    }

    fn resolve_qualified(
        &self,
        owner: &str,
        name: &str,
        model: &SelectModel,
    ) -> Result<String, QueryError> {
        validate_identifier(owner)?;
        validate_identifier(name)?;
        if owner == model.root_alias || owner == model.scan_root_alias {
            return Ok(format!("{}.{}", model.root_alias, name));
        }
        if model.references.contains_key(owner) {
            return Ok(format!("{owner}.{name}"));
        }
        Err(QueryError::UnknownAlias(owner.to_string()))
    }
}
