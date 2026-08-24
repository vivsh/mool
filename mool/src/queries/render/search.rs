//! Rendering and defensive metadata validation for search expressions.

#[cfg(feature = "fuzzy-search")]
use super::super::expr::FuzzyOperation;
#[cfg(feature = "vector")]
use super::super::expr::VectorOperation;
use super::super::expr::{ColumnRef, ExprNode};
#[cfg(feature = "text-search")]
use super::super::expr::{TextQueryMode, TextSearchResult};
use super::super::handles::{ColumnOwner, Table};
use super::super::validate::table_name;
#[cfg(feature = "text-search")]
use super::super::validate::validate_identifier;
use super::{RenderMode, Renderer};
use crate::{QueryError, SqlDialect};

#[cfg(feature = "text-search")]
impl Renderer {
    pub(super) fn render_text_search(
        &mut self,
        table: &Table,
        name: &str,
        query: &ExprNode,
        result: TextSearchResult,
        query_mode: TextQueryMode,
        render_mode: RenderMode<'_>,
    ) -> Result<String, QueryError> {
        let definition = resolve_full_text(table, name)?;
        validate_full_text(table, definition)?;
        match self.dialect() {
            SqlDialect::Postgres => self.render_postgres_text_search(
                table,
                definition,
                query,
                result,
                query_mode,
                render_mode,
            ),
            SqlDialect::Mysql | SqlDialect::Mariadb => self.render_mysql_text_search(
                table,
                definition,
                query,
                result,
                query_mode,
                render_mode,
            ),
            SqlDialect::Sqlite => Err(QueryError::Unsupported {
                dialect: "sqlite",
                feature: "full-text search",
            }),
        }
    }

    fn render_postgres_text_search(
        &mut self,
        table: &Table,
        definition: &crate::search::FullTextDefinition,
        query: &ExprNode,
        result: TextSearchResult,
        query_mode: TextQueryMode,
        render_mode: RenderMode<'_>,
    ) -> Result<String, QueryError> {
        validate_config(&definition.postgres.config)?;
        let document = postgres_document(self, table, &definition.columns, render_mode)?;
        let query = self.render_expr(query, render_mode)?;
        let parser = match query_mode {
            TextQueryMode::Plain => "plainto_tsquery",
            TextQueryMode::Phrase => "phraseto_tsquery",
            TextQueryMode::WebSearch => "websearch_to_tsquery",
            TextQueryMode::NaturalLanguage | TextQueryMode::Boolean => {
                return Err(QueryError::Unsupported {
                    dialect: "postgres",
                    feature: "MySQL full-text query mode",
                });
            }
        };
        let config = &definition.postgres.config;
        let vector = format!("to_tsvector('{config}'::regconfig, {document})");
        let terms = format!("{parser}('{config}'::regconfig, {query})");
        Ok(match result {
            TextSearchResult::Matches => format!("({vector} @@ {terms})"),
            TextSearchResult::Relevance => {
                format!("CAST(ts_rank_cd({vector}, {terms}) AS double precision)")
            }
        })
    }

    fn render_mysql_text_search(
        &mut self,
        table: &Table,
        definition: &crate::search::FullTextDefinition,
        query: &ExprNode,
        result: TextSearchResult,
        query_mode: TextQueryMode,
        render_mode: RenderMode<'_>,
    ) -> Result<String, QueryError> {
        let columns = render_columns(self, table, &definition.columns, render_mode)?.join(", ");
        let query = self.render_expr(query, render_mode)?;
        let mode = match query_mode {
            TextQueryMode::NaturalLanguage => "IN NATURAL LANGUAGE MODE",
            TextQueryMode::Boolean => "IN BOOLEAN MODE",
            TextQueryMode::Plain | TextQueryMode::Phrase | TextQueryMode::WebSearch => {
                return Err(QueryError::Unsupported {
                    dialect: self.dialect().name(),
                    feature: "PostgreSQL full-text query mode",
                });
            }
        };
        let expression = format!("MATCH ({columns}) AGAINST ({query} {mode})");
        Ok(match result {
            TextSearchResult::Matches => format!("({expression} > 0)"),
            TextSearchResult::Relevance => expression,
        })
    }
}

#[cfg(feature = "fuzzy-search")]
impl Renderer {
    pub(super) fn render_fuzzy(
        &mut self,
        column: &ColumnRef,
        query: &ExprNode,
        operation: FuzzyOperation,
        threshold: Option<&ExprNode>,
        mode: RenderMode<'_>,
    ) -> Result<String, QueryError> {
        if self.dialect() != SqlDialect::Postgres {
            return Err(QueryError::Unsupported {
                dialect: self.dialect().name(),
                feature: "trigram fuzzy search",
            });
        }
        validate_declared_column(column, crate::search::SearchCapability::Fuzzy)?;
        let column = self.render_expr(&ExprNode::Column(column.clone()), mode)?;
        let query = self.render_expr(query, mode)?;
        Ok(match operation {
            FuzzyOperation::Similarity => {
                format!("CAST(similarity({column}, {query}) AS double precision)")
            }
            FuzzyOperation::Distance => format!("({column} <-> {query})"),
            FuzzyOperation::MinimumSimilarity => {
                let threshold = threshold.ok_or_else(|| QueryError::InvalidSearchMetadata {
                    table: column.clone(),
                    name: column.clone(),
                    reason: "minimum similarity requires a threshold".to_string(),
                })?;
                let threshold = self.render_expr(threshold, mode)?;
                format!("(CAST(similarity({column}, {query}) AS double precision) >= {threshold})")
            }
            FuzzyOperation::ThresholdMatch => format!("({column} % {query})"),
            FuzzyOperation::WordSimilarity => {
                format!("CAST(word_similarity({column}, {query}) AS double precision)")
            }
            FuzzyOperation::StrictWordSimilarity => {
                format!("CAST(strict_word_similarity({column}, {query}) AS double precision)")
            }
        })
    }
}

#[cfg(feature = "vector")]
impl Renderer {
    pub(super) fn render_vector_distance(
        &mut self,
        column: &ColumnRef,
        query: &ExprNode,
        operation: VectorOperation,
        mode: RenderMode<'_>,
    ) -> Result<String, QueryError> {
        validate_vector_column(column)?;
        let column = self.render_expr(&ExprNode::Column(column.clone()), mode)?;
        let query = self.render_expr(query, mode)?;
        match self.dialect() {
            SqlDialect::Postgres => Ok(match operation {
                VectorOperation::L2Distance => format!("({column} <-> {query})"),
                VectorOperation::CosineDistance => format!("({column} <=> {query})"),
                VectorOperation::CosineSimilarity => format!("(1.0 - ({column} <=> {query}))"),
                VectorOperation::InnerProduct => format!("({column} <#> {query})"),
                VectorOperation::L1Distance => format!("({column} <+> {query})"),
            }),
            SqlDialect::Mariadb => match operation {
                VectorOperation::L2Distance => {
                    Ok(format!("VEC_DISTANCE_EUCLIDEAN({column}, {query})"))
                }
                VectorOperation::CosineDistance => {
                    Ok(format!("VEC_DISTANCE_COSINE({column}, {query})"))
                }
                VectorOperation::CosineSimilarity => {
                    Ok(format!("(1.0 - VEC_DISTANCE_COSINE({column}, {query}))"))
                }
                VectorOperation::InnerProduct | VectorOperation::L1Distance => {
                    Err(QueryError::Unsupported {
                        dialect: "mariadb",
                        feature: "PostgreSQL vector operator",
                    })
                }
            },
            SqlDialect::Mysql | SqlDialect::Sqlite => Err(QueryError::Unsupported {
                dialect: self.dialect().name(),
                feature: "vector similarity",
            }),
        }
    }
}

#[cfg(feature = "text-search")]
fn resolve_full_text<'a>(
    table: &'a Table,
    name: &str,
) -> Result<&'a crate::search::FullTextDefinition, QueryError> {
    let table_name = qualified_table_name(table);
    if table.data.search.identity_count(name) > 1 {
        return Err(QueryError::DuplicateSearchDefinition {
            table: table_name,
            name: name.to_string(),
        });
    }
    let mut matches = table
        .data
        .search
        .full_text
        .iter()
        .filter(|definition| definition.name == name);
    let Some(definition) = matches.next() else {
        return match table.data.search.capability_named(name) {
            Some(capability) => Err(QueryError::SearchCapabilityMismatch {
                table: table_name,
                name: name.to_string(),
                requested: crate::search::SearchCapability::FullText.label(),
                declared: capability.label(),
            }),
            None => Err(QueryError::UnknownSearchDefinition {
                table: table_name,
                name: name.to_string(),
            }),
        };
    };
    if matches.next().is_some() {
        return Err(QueryError::DuplicateSearchDefinition {
            table: table_name,
            name: name.to_string(),
        });
    }
    Ok(definition)
}

#[cfg(feature = "text-search")]
fn validate_full_text(
    table: &Table,
    definition: &crate::search::FullTextDefinition,
) -> Result<(), QueryError> {
    validate_metadata_identifier(table, &definition.name, &definition.name)?;
    if definition.columns.is_empty() {
        return invalid_metadata(table, &definition.name, "columns cannot be empty");
    }
    let mut seen = std::collections::HashSet::new();
    for column in &definition.columns {
        validate_metadata_identifier(table, &definition.name, column)?;
        if !seen.insert(column) {
            return invalid_metadata(
                table,
                &definition.name,
                &format!("column '{column}' is declared more than once"),
            );
        }
        if let Some(columns) = &table.data.columns
            && !columns.iter().any(|candidate| candidate == column)
        {
            return invalid_metadata(
                table,
                &definition.name,
                &format!("unknown column '{column}'"),
            );
        }
    }
    if let Some(index_name) = &definition.index_name {
        validate_metadata_identifier(table, &definition.name, index_name)?;
    }
    validate_config(&definition.postgres.config).map_err(|error| {
        QueryError::InvalidSearchMetadata {
            table: qualified_table_name(table),
            name: definition.name.clone(),
            reason: error.to_string(),
        }
    })?;
    Ok(())
}

#[cfg(feature = "text-search")]
fn validate_metadata_identifier(table: &Table, name: &str, value: &str) -> Result<(), QueryError> {
    validate_identifier(value).map_err(|error| QueryError::InvalidSearchMetadata {
        table: qualified_table_name(table),
        name: name.to_string(),
        reason: error.to_string(),
    })
}

#[cfg(feature = "text-search")]
fn postgres_document(
    renderer: &mut Renderer,
    table: &Table,
    columns: &[String],
    mode: RenderMode<'_>,
) -> Result<String, QueryError> {
    Ok(render_columns(renderer, table, columns, mode)?
        .into_iter()
        .map(|column| format!("coalesce({column}, '')"))
        .collect::<Vec<_>>()
        .join(" || ' ' || "))
}

#[cfg(feature = "text-search")]
fn render_columns(
    renderer: &mut Renderer,
    table: &Table,
    columns: &[String],
    mode: RenderMode<'_>,
) -> Result<Vec<String>, QueryError> {
    columns
        .iter()
        .map(|name| {
            renderer.render_expr(
                &ExprNode::Column(ColumnRef {
                    owner: ColumnOwner::Root(table.clone()),
                    name: name.as_str().into(),
                }),
                mode,
            )
        })
        .collect()
}

#[cfg(any(feature = "fuzzy-search", feature = "vector"))]
fn validate_declared_column(
    column: &ColumnRef,
    capability: crate::search::SearchCapability,
) -> Result<(), QueryError> {
    let ColumnOwner::Root(table) = &column.owner else {
        return Err(QueryError::InvalidSearchMetadata {
            table: "non-table source".to_string(),
            name: column.name.to_string(),
            reason: "search extensions require a model-table column".to_string(),
        });
    };
    let declared = declared_column_count(table, column, capability);
    if declared > 1 {
        return Err(QueryError::DuplicateSearchDefinition {
            table: qualified_table_name(table),
            name: column.name.to_string(),
        });
    }
    if declared == 1 {
        return Ok(());
    }
    if let Some(declared) = table.data.search.capability_named(&column.name) {
        return Err(QueryError::SearchCapabilityMismatch {
            table: qualified_table_name(table),
            name: column.name.to_string(),
            requested: capability.label(),
            declared: declared.label(),
        });
    }
    Err(QueryError::UnknownSearchDefinition {
        table: qualified_table_name(table),
        name: column.name.to_string(),
    })
}

#[cfg(any(feature = "fuzzy-search", feature = "vector"))]
fn declared_column_count(
    table: &Table,
    column: &ColumnRef,
    capability: crate::search::SearchCapability,
) -> usize {
    match capability {
        #[cfg(feature = "text-search")]
        crate::search::SearchCapability::FullText => 0,
        #[cfg(feature = "fuzzy-search")]
        crate::search::SearchCapability::Fuzzy => table
            .data
            .search
            .fuzzy
            .iter()
            .filter(|definition| definition.column == column.name.as_ref())
            .count(),
        #[cfg(feature = "vector")]
        crate::search::SearchCapability::Vector => table
            .data
            .search
            .vectors
            .iter()
            .filter(|definition| definition.column == column.name.as_ref())
            .count(),
    }
}

#[cfg(feature = "vector")]
fn validate_vector_column(column: &ColumnRef) -> Result<(), QueryError> {
    validate_declared_column(column, crate::search::SearchCapability::Vector)
}

#[cfg(feature = "text-search")]
fn validate_config(config: &str) -> Result<(), QueryError> {
    let mut parts = config.split('.');
    let first = parts.next().unwrap_or_default();
    validate_identifier(first)?;
    if let Some(second) = parts.next() {
        validate_identifier(second)?;
    }
    if parts.next().is_some() {
        return Err(QueryError::InvalidIdentifier(config.to_string()));
    }
    Ok(())
}

fn qualified_table_name(table: &Table) -> String {
    table_name(table.data.schema.as_deref(), &table.data.name)
}

#[cfg(any(feature = "text-search", feature = "vector"))]
#[cfg(feature = "text-search")]
fn invalid_metadata<T>(table: &Table, name: &str, reason: &str) -> Result<T, QueryError> {
    Err(QueryError::InvalidSearchMetadata {
        table: qualified_table_name(table),
        name: name.to_string(),
        reason: reason.to_string(),
    })
}
