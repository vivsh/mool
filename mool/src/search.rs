//! Search capability metadata shared by model schemas and typed queries.

use std::fmt;

/// Logical search capability attached to a table definition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchCapability {
    /// A named multi-column full-text definition.
    #[cfg(feature = "text-search")]
    FullText,
    /// Trigram similarity on one text column.
    #[cfg(feature = "fuzzy-search")]
    Fuzzy,
    /// Vector distance on one fixed-dimension column.
    #[cfg(feature = "vector")]
    Vector,
}

impl SearchCapability {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            #[cfg(feature = "text-search")]
            Self::FullText => "full-text search",
            #[cfg(feature = "fuzzy-search")]
            Self::Fuzzy => "fuzzy search",
            #[cfg(feature = "vector")]
            Self::Vector => "vector search",
        }
    }
}

impl fmt::Display for SearchCapability {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.label())
    }
}

/// PostgreSQL options for one full-text definition.
#[cfg(feature = "text-search")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresFullTextOptions {
    pub(crate) config: String,
}

#[cfg(feature = "text-search")]
impl Default for PostgresFullTextOptions {
    fn default() -> Self {
        Self {
            config: "simple".to_string(),
        }
    }
}

/// Named full-text metadata attached to an existing set of text columns.
#[cfg(feature = "text-search")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FullTextDefinition {
    pub(crate) name: String,
    pub(crate) columns: Vec<String>,
    pub(crate) index_name: Option<String>,
    pub(crate) postgres: PostgresFullTextOptions,
}

#[cfg(feature = "text-search")]
impl FullTextDefinition {
    /// Creates a portable full-text definition using PostgreSQL's `simple` configuration.
    pub fn new(
        name: impl Into<String>,
        columns: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            name: name.into(),
            columns: columns.into_iter().map(Into::into).collect(),
            index_name: None,
            postgres: PostgresFullTextOptions::default(),
        }
    }

    /// Overrides the physical database index name without changing the logical search name.
    pub fn index_name(mut self, name: impl Into<String>) -> Self {
        self.index_name = Some(name.into());
        self
    }

    /// Selects the PostgreSQL text-search configuration used by the index and queries.
    pub fn postgres_config(mut self, config: impl Into<String>) -> Self {
        self.postgres.config = config.into();
        self
    }

    /// Returns the logical name used by table-bound query methods.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the existing text columns participating in this definition.
    pub fn columns(&self) -> &[String] {
        &self.columns
    }

    /// Returns the explicit physical index name, if configured.
    pub fn physical_index_name(&self) -> Option<&str> {
        self.index_name.as_deref()
    }
}

/// PostgreSQL trigram index method.
#[cfg(feature = "fuzzy-search")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FuzzyIndex {
    /// Do not create an index; similarity remains exact.
    #[default]
    None,
    /// GiST trigram index, including nearest-neighbour distance ordering.
    GiSt,
    /// GIN trigram index for indexed similarity predicates.
    Gin,
}

/// Fuzzy-search metadata for one existing text column.
#[cfg(feature = "fuzzy-search")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FuzzyDefinition {
    pub(crate) column: String,
    pub(crate) index: FuzzyIndex,
    pub(crate) index_name: Option<String>,
}

#[cfg(feature = "fuzzy-search")]
impl FuzzyDefinition {
    /// Declares trigram operations for one text column without creating an index.
    pub fn new(column: impl Into<String>) -> Self {
        Self {
            column: column.into(),
            index: FuzzyIndex::None,
            index_name: None,
        }
    }

    /// Selects the PostgreSQL trigram index method.
    pub fn index(mut self, index: FuzzyIndex) -> Self {
        self.index = index;
        self
    }

    /// Overrides the physical trigram index name.
    pub fn index_name(mut self, name: impl Into<String>) -> Self {
        self.index_name = Some(name.into());
        self
    }
}

/// Portable vector distance metric.
#[cfg(feature = "vector")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VectorMetric {
    /// Cosine distance, suitable for directional embedding similarity.
    #[default]
    Cosine,
    /// Euclidean L2 distance.
    L2,
}

/// Approximate vector index family.
#[cfg(feature = "vector")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VectorIndex {
    /// Keep exact scans and do not create an ANN index.
    #[default]
    None,
    /// Hierarchical navigable small world index.
    Hnsw,
    /// PostgreSQL-only IVFFlat index.
    IvfFlat,
}

/// PostgreSQL-specific vector-index tuning.
#[cfg(feature = "vector")]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PostgresVectorOptions {
    pub(crate) m: Option<u16>,
    pub(crate) ef_construction: Option<u16>,
    pub(crate) lists: Option<u32>,
}

/// MariaDB-specific vector-index tuning.
#[cfg(feature = "vector")]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MariaDbVectorOptions {
    pub(crate) m: Option<u16>,
}

/// Vector column and optional ANN-index metadata.
#[cfg(feature = "vector")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VectorDefinition {
    pub(crate) column: String,
    pub(crate) dimensions: Option<usize>,
    pub(crate) metric: VectorMetric,
    pub(crate) index: VectorIndex,
    pub(crate) index_name: Option<String>,
    pub(crate) postgres: PostgresVectorOptions,
    pub(crate) mariadb: MariaDbVectorOptions,
}

#[cfg(feature = "vector")]
impl VectorDefinition {
    /// Declares a dynamically sized vector column with exact cosine search.
    pub fn new(column: impl Into<String>) -> Self {
        Self {
            column: column.into(),
            dimensions: None,
            metric: VectorMetric::Cosine,
            index: VectorIndex::None,
            index_name: None,
            postgres: PostgresVectorOptions::default(),
            mariadb: MariaDbVectorOptions::default(),
        }
    }

    /// Constrains the physical vector column to one dimension.
    pub fn dimensions(mut self, dimensions: usize) -> Self {
        self.dimensions = Some(dimensions);
        self
    }

    /// Selects the vector metric used by an optional ANN index.
    pub fn metric(mut self, metric: VectorMetric) -> Self {
        self.metric = metric;
        self
    }

    /// Selects an approximate index family.
    pub fn index(mut self, index: VectorIndex) -> Self {
        self.index = index;
        self
    }

    /// Overrides the physical vector-index name.
    pub fn index_name(mut self, name: impl Into<String>) -> Self {
        self.index_name = Some(name.into());
        self
    }

    /// Sets PostgreSQL HNSW construction parameters.
    pub fn postgres_hnsw(mut self, m: Option<u16>, ef_construction: Option<u16>) -> Self {
        self.postgres.m = m;
        self.postgres.ef_construction = ef_construction;
        self
    }

    /// Sets the PostgreSQL IVFFlat list count.
    pub fn postgres_lists(mut self, lists: u32) -> Self {
        self.postgres.lists = Some(lists);
        self
    }

    /// Sets the MariaDB HNSW connection count.
    pub fn mariadb_m(mut self, m: u16) -> Self {
        self.mariadb.m = Some(m);
        self
    }
}

/// Immutable search metadata carried by a model-bound table handle.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SearchTableMetadata {
    #[cfg(feature = "text-search")]
    pub(crate) full_text: Vec<FullTextDefinition>,
    #[cfg(feature = "fuzzy-search")]
    pub(crate) fuzzy: Vec<FuzzyDefinition>,
    #[cfg(feature = "vector")]
    pub(crate) vectors: Vec<VectorDefinition>,
}

impl SearchTableMetadata {
    /// Creates empty search metadata.
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends one named full-text definition.
    #[cfg(feature = "text-search")]
    pub fn full_text(mut self, definition: FullTextDefinition) -> Self {
        self.full_text.push(definition);
        self
    }

    /// Appends one fuzzy-search column definition.
    #[cfg(feature = "fuzzy-search")]
    pub fn fuzzy(mut self, definition: FuzzyDefinition) -> Self {
        self.fuzzy.push(definition);
        self
    }

    /// Appends one vector-column definition.
    #[cfg(feature = "vector")]
    pub fn vector(mut self, definition: VectorDefinition) -> Self {
        self.vectors.push(definition);
        self
    }

    pub(crate) fn capability_named(&self, name: &str) -> Option<SearchCapability> {
        #[cfg(feature = "text-search")]
        if self
            .full_text
            .iter()
            .any(|definition| definition.name == name)
        {
            return Some(SearchCapability::FullText);
        }
        #[cfg(feature = "fuzzy-search")]
        if self
            .fuzzy
            .iter()
            .any(|definition| definition.column == name)
        {
            return Some(SearchCapability::Fuzzy);
        }
        #[cfg(feature = "vector")]
        if self
            .vectors
            .iter()
            .any(|definition| definition.column == name)
        {
            return Some(SearchCapability::Vector);
        }
        None
    }

    #[cfg(feature = "text-search")]
    pub(crate) fn identity_count(&self, name: &str) -> usize {
        let mut count = 0;
        #[cfg(feature = "text-search")]
        {
            count += self
                .full_text
                .iter()
                .filter(|definition| definition.name == name)
                .count();
        }
        #[cfg(feature = "fuzzy-search")]
        {
            count += self
                .fuzzy
                .iter()
                .filter(|definition| definition.column == name)
                .count();
        }
        #[cfg(feature = "vector")]
        {
            count += self
                .vectors
                .iter()
                .filter(|definition| definition.column == name)
                .count();
        }
        count
    }
}

/// Macro-facing schema generation helpers.
#[doc(hidden)]
pub mod __private {
    use gaman::core::Dialect;

    use super::SearchTableMetadata;

    /// Returns deterministic advanced-index declarations for top-level opaque registration.
    pub fn index_statements(
        table_name: &str,
        table_schema: Option<&str>,
        metadata: &SearchTableMetadata,
        dialect: &Dialect,
    ) -> Vec<String> {
        #[allow(unused_mut)]
        let mut statements = Vec::new();
        #[cfg(feature = "text-search")]
        for definition in &metadata.full_text {
            if let Some((_, sql)) = full_text_index(table_name, table_schema, definition, *dialect)
            {
                statements.push(sql);
            }
        }
        #[cfg(feature = "fuzzy-search")]
        for definition in &metadata.fuzzy {
            if let Some((_, sql)) = fuzzy_index(table_name, table_schema, definition, *dialect) {
                statements.push(sql);
            }
        }
        #[cfg(feature = "vector")]
        for definition in &metadata.vectors {
            if let Some((_, sql)) = vector_index(table_name, table_schema, definition, *dialect) {
                statements.push(sql);
            }
        }
        statements
    }

    /// Returns extensions that must exist before model tables or indexes are created.
    pub fn required_extensions(
        metadata: &SearchTableMetadata,
        dialect: Dialect,
    ) -> Vec<&'static str> {
        if dialect != Dialect::Postgres {
            return Vec::new();
        }
        #[allow(unused_mut)]
        let mut extensions = Vec::new();
        let _ = metadata;
        #[cfg(feature = "fuzzy-search")]
        if !metadata.fuzzy.is_empty() {
            extensions.push("pg_trgm");
        }
        #[cfg(feature = "vector")]
        if !metadata.vectors.is_empty() {
            extensions.push("vector");
        }
        extensions
    }

    /// Validates capability metadata against the selected schema dialect.
    pub fn validate_dialect(
        metadata: &SearchTableMetadata,
        dialect: Dialect,
    ) -> Result<(), String> {
        validate_metadata(metadata)?;
        #[cfg(feature = "text-search")]
        if !metadata.full_text.is_empty()
            && !matches!(
                dialect,
                Dialect::Postgres | Dialect::Mysql | Dialect::Mariadb
            )
        {
            return Err("full-text search is unsupported for sqlite".to_string());
        }
        #[cfg(feature = "fuzzy-search")]
        if !metadata.fuzzy.is_empty() && dialect != Dialect::Postgres {
            return Err(format!(
                "trigram fuzzy search is unsupported for {dialect:?}"
            ));
        }
        #[cfg(feature = "vector")]
        if !metadata.vectors.is_empty() && !matches!(dialect, Dialect::Postgres | Dialect::Mariadb)
        {
            return Err(format!("vector similarity is unsupported for {dialect:?}"));
        }
        #[cfg(feature = "vector")]
        for definition in &metadata.vectors {
            validate_vector_definition(definition, dialect)?;
        }
        Ok(())
    }

    fn validate_metadata(metadata: &SearchTableMetadata) -> Result<(), String> {
        let mut identities = std::collections::HashSet::new();
        #[cfg(feature = "text-search")]
        for definition in &metadata.full_text {
            validate_full_text_definition(definition, &mut identities)?;
        }
        #[cfg(feature = "fuzzy-search")]
        for definition in &metadata.fuzzy {
            validate_fuzzy_definition(definition, &mut identities)?;
        }
        #[cfg(feature = "vector")]
        for definition in &metadata.vectors {
            validate_vector_metadata(definition, &mut identities)?;
        }
        Ok(())
    }

    #[cfg(feature = "text-search")]
    fn validate_full_text_definition(
        definition: &super::FullTextDefinition,
        identities: &mut std::collections::HashSet<String>,
    ) -> Result<(), String> {
        validate_identity(&definition.name, identities)?;
        if definition.columns.is_empty() {
            return Err(format!(
                "full-text definition '{}' has no columns",
                definition.name
            ));
        }
        let mut columns = std::collections::HashSet::new();
        for column in &definition.columns {
            validate_identifier(column)?;
            if !columns.insert(column) {
                return Err(format!(
                    "full-text definition '{}' repeats column '{column}'",
                    definition.name
                ));
            }
        }
        validate_optional_identifier(definition.index_name.as_deref())?;
        validate_qualified_identifier(&definition.postgres.config)
    }

    #[cfg(feature = "fuzzy-search")]
    fn validate_fuzzy_definition(
        definition: &super::FuzzyDefinition,
        identities: &mut std::collections::HashSet<String>,
    ) -> Result<(), String> {
        validate_identity(&definition.column, identities)?;
        validate_optional_identifier(definition.index_name.as_deref())
    }

    #[cfg(feature = "vector")]
    fn validate_vector_metadata(
        definition: &super::VectorDefinition,
        identities: &mut std::collections::HashSet<String>,
    ) -> Result<(), String> {
        validate_identity(&definition.column, identities)?;
        validate_optional_identifier(definition.index_name.as_deref())?;
        if definition.dimensions == Some(0) {
            return Err(format!(
                "vector column '{}' has zero dimensions",
                definition.column
            ));
        }
        validate_vector_options(definition)
    }

    #[cfg(feature = "vector")]
    fn validate_vector_options(definition: &super::VectorDefinition) -> Result<(), String> {
        if definition.index != super::VectorIndex::None && definition.dimensions.is_none() {
            return Err("vector ANN indexes require a declared dimension".to_string());
        }
        match definition.index {
            super::VectorIndex::None => {
                if definition.postgres != super::PostgresVectorOptions::default()
                    || definition.mariadb != super::MariaDbVectorOptions::default()
                {
                    return Err("vector index options require an ANN index".to_string());
                }
            }
            super::VectorIndex::Hnsw if definition.postgres.lists.is_some() => {
                return Err("PostgreSQL lists is only valid for IVFFlat".to_string());
            }
            super::VectorIndex::IvfFlat if definition.postgres.lists.unwrap_or(0) == 0 => {
                return Err("IVFFlat requires a positive PostgreSQL lists value".to_string());
            }
            super::VectorIndex::IvfFlat
                if definition.postgres.m.is_some()
                    || definition.postgres.ef_construction.is_some() =>
            {
                return Err("PostgreSQL HNSW options are invalid for IVFFlat".to_string());
            }
            _ => {}
        }
        if let Some(m) = definition.mariadb.m
            && !(3..=200).contains(&m)
        {
            return Err("MariaDB vector m must be between 3 and 200".to_string());
        }
        Ok(())
    }

    fn validate_identity(
        identity: &str,
        identities: &mut std::collections::HashSet<String>,
    ) -> Result<(), String> {
        validate_identifier(identity)?;
        if identities.insert(identity.to_string()) {
            return Ok(());
        }
        Err(format!("duplicate search definition '{identity}'"))
    }

    fn validate_optional_identifier(value: Option<&str>) -> Result<(), String> {
        match value {
            Some(value) => validate_identifier(value),
            None => Ok(()),
        }
    }

    #[cfg(feature = "text-search")]
    fn validate_qualified_identifier(value: &str) -> Result<(), String> {
        let mut parts = value.split('.');
        let Some(first) = parts.next() else {
            return Err("identifier cannot be empty".to_string());
        };
        validate_identifier(first)?;
        if let Some(second) = parts.next() {
            validate_identifier(second)?;
        }
        if parts.next().is_some() {
            return Err(format!("invalid qualified identifier '{value}'"));
        }
        Ok(())
    }

    fn validate_identifier(value: &str) -> Result<(), String> {
        let mut chars = value.chars();
        let Some(first) = chars.next() else {
            return Err("identifier cannot be empty".to_string());
        };
        if !(first == '_' || first.is_ascii_alphabetic())
            || !chars.all(|character| character == '_' || character.is_ascii_alphanumeric())
        {
            return Err(format!("invalid SQL identifier '{value}'"));
        }
        Ok(())
    }

    #[cfg(feature = "text-search")]
    fn full_text_index(
        table: &str,
        schema: Option<&str>,
        definition: &super::FullTextDefinition,
        dialect: Dialect,
    ) -> Option<(String, String)> {
        let name = definition
            .index_name
            .clone()
            .unwrap_or_else(|| format!("{table}_{}_fts_idx", definition.name));
        match dialect {
            Dialect::Postgres => {
                let columns = definition
                    .columns
                    .iter()
                    .map(|column| format!("coalesce({}, '')", quote_ident(column, dialect)))
                    .collect::<Vec<_>>()
                    .join(" || ' ' || ");
                let sql = format!(
                    "CREATE INDEX {} ON {} USING GIN (to_tsvector('{}'::regconfig, {}))",
                    quote_ident(&name, dialect),
                    qualified_table(table, schema, dialect),
                    definition.postgres.config,
                    columns
                );
                Some((name, sql))
            }
            Dialect::Mysql | Dialect::Mariadb => {
                let columns = definition
                    .columns
                    .iter()
                    .map(|column| quote_ident(column, dialect))
                    .collect::<Vec<_>>()
                    .join(", ");
                let sql = format!(
                    "CREATE FULLTEXT INDEX {} ON {} ({})",
                    quote_ident(&name, dialect),
                    qualified_table(table, schema, dialect),
                    columns
                );
                Some((name, sql))
            }
            Dialect::Sqlite => None,
        }
    }

    #[cfg(feature = "fuzzy-search")]
    fn fuzzy_index(
        table: &str,
        schema: Option<&str>,
        definition: &super::FuzzyDefinition,
        dialect: Dialect,
    ) -> Option<(String, String)> {
        if dialect != Dialect::Postgres || definition.index == super::FuzzyIndex::None {
            return None;
        }
        let name = definition
            .index_name
            .clone()
            .unwrap_or_else(|| format!("{table}_{}_trgm_idx", definition.column));
        let method = match definition.index {
            super::FuzzyIndex::GiSt => "gist",
            super::FuzzyIndex::Gin => "gin",
            super::FuzzyIndex::None => return None,
        };
        let sql = format!(
            "CREATE INDEX {} ON {} USING {} ({} {}_trgm_ops)",
            quote_ident(&name, dialect),
            qualified_table(table, schema, dialect),
            method,
            quote_ident(&definition.column, dialect),
            method
        );
        Some((name, sql))
    }

    #[cfg(feature = "vector")]
    fn vector_index(
        table: &str,
        schema: Option<&str>,
        definition: &super::VectorDefinition,
        dialect: Dialect,
    ) -> Option<(String, String)> {
        if definition.index == super::VectorIndex::None {
            return None;
        }
        let metric = match definition.metric {
            super::VectorMetric::Cosine => "cosine",
            super::VectorMetric::L2 => "l2",
        };
        let name = definition
            .index_name
            .clone()
            .unwrap_or_else(|| format!("{table}_{}_{metric}_vec_idx", definition.column));
        let sql = match dialect {
            Dialect::Postgres => postgres_vector_index(table, schema, &name, definition),
            Dialect::Mariadb => mariadb_vector_index(table, schema, &name, definition),
            Dialect::Mysql | Dialect::Sqlite => return None,
        };
        Some((name, sql))
    }

    #[cfg(feature = "vector")]
    fn postgres_vector_index(
        table: &str,
        schema: Option<&str>,
        name: &str,
        definition: &super::VectorDefinition,
    ) -> String {
        let method = match definition.index {
            super::VectorIndex::Hnsw => "hnsw",
            super::VectorIndex::IvfFlat => "ivfflat",
            super::VectorIndex::None => "hnsw",
        };
        let ops = match definition.metric {
            super::VectorMetric::Cosine => "vector_cosine_ops",
            super::VectorMetric::L2 => "vector_l2_ops",
        };
        let mut options = Vec::new();
        if method == "hnsw" {
            if let Some(m) = definition.postgres.m {
                options.push(format!("m = {m}"));
            }
            if let Some(ef) = definition.postgres.ef_construction {
                options.push(format!("ef_construction = {ef}"));
            }
        } else if let Some(lists) = definition.postgres.lists {
            options.push(format!("lists = {lists}"));
        }
        let options = if options.is_empty() {
            String::new()
        } else {
            format!(" WITH ({})", options.join(", "))
        };
        format!(
            "CREATE INDEX {} ON {} USING {} ({} {}){}",
            quote_ident(name, Dialect::Postgres),
            qualified_table(table, schema, Dialect::Postgres),
            method,
            quote_ident(&definition.column, Dialect::Postgres),
            ops,
            options
        )
    }

    #[cfg(feature = "vector")]
    fn mariadb_vector_index(
        table: &str,
        schema: Option<&str>,
        name: &str,
        definition: &super::VectorDefinition,
    ) -> String {
        let metric = match definition.metric {
            super::VectorMetric::Cosine => "COSINE",
            super::VectorMetric::L2 => "EUCLIDEAN",
        };
        let m = definition
            .mariadb
            .m
            .map(|value| format!(" M={value}"))
            .unwrap_or_default();
        format!(
            "CREATE VECTOR INDEX {} ON {} ({}){} DISTANCE={}",
            quote_ident(name, Dialect::Mariadb),
            qualified_table(table, schema, Dialect::Mariadb),
            quote_ident(&definition.column, Dialect::Mariadb),
            m,
            metric
        )
    }

    #[cfg(feature = "vector")]
    fn validate_vector_definition(
        definition: &super::VectorDefinition,
        dialect: Dialect,
    ) -> Result<(), String> {
        if definition.dimensions == Some(0) {
            return Err(format!(
                "vector column '{}' has zero dimensions",
                definition.column
            ));
        }
        if dialect == Dialect::Mariadb && definition.dimensions.is_none() {
            return Err(format!(
                "MariaDB vector column '{}' requires dimensions",
                definition.column
            ));
        }
        if dialect == Dialect::Postgres
            && definition.index != super::VectorIndex::None
            && definition
                .dimensions
                .is_some_and(|dimensions| dimensions > 2_000)
        {
            return Err(format!(
                "PostgreSQL vector index '{}' exceeds 2000 dimensions",
                definition.column
            ));
        }
        if dialect == Dialect::Mariadb
            && definition
                .dimensions
                .is_some_and(|dimensions| dimensions > 16_383)
        {
            return Err(format!(
                "MariaDB vector column '{}' exceeds 16383 dimensions",
                definition.column
            ));
        }
        if dialect == Dialect::Mariadb && definition.index == super::VectorIndex::IvfFlat {
            return Err("MariaDB does not support IVFFlat vector indexes".to_string());
        }
        Ok(())
    }

    fn qualified_table(table: &str, schema: Option<&str>, dialect: Dialect) -> String {
        match schema {
            Some(schema) => format!(
                "{}.{}",
                quote_ident(schema, dialect),
                quote_ident(table, dialect)
            ),
            None => quote_ident(table, dialect),
        }
    }

    fn quote_ident(value: &str, dialect: Dialect) -> String {
        match dialect {
            Dialect::Postgres | Dialect::Sqlite => format!("\"{value}\""),
            Dialect::Mysql | Dialect::Mariadb => format!("`{value}`"),
        }
    }
}
