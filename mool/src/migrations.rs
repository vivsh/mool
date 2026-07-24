#[cfg(feature = "migrations")]
use std::{
    collections::{BTreeMap, HashSet},
    path::{Path, PathBuf},
};

#[cfg(feature = "migrations")]
use thiserror::Error;

#[cfg(feature = "migrations")]
use gaman::{core::MigrationStore, runner_factory::DirectoryMigrationStore};

/// Command-runner types for applications that integrate Mool migrations.
#[cfg(feature = "migrations")]
pub mod engine;

#[cfg(feature = "migrations")]
pub use gaman::EmbeddedMigrations;
pub use gaman::core::Dialect;
pub use gaman::schema::{
    Column, ColumnDesc, ColumnRef, ColumnType, Constraint, FunctionDef, Index, IntoTable,
    SchemaLoadError, Table,
};
pub use gaman::schema::{Schema, SchemaBuilder, TableBuilder};
#[cfg(feature = "migrations")]
pub use mool_macros::embedded_migrations;

/// A crate-level migration history registered through a bundle.
#[cfg(feature = "migrations")]
#[derive(Clone, Copy)]
pub struct MigrationSource {
    namespace: Option<&'static str>,
    migrations: &'static EmbeddedMigrations,
}

/// A fallible static schema builder registered with a migration namespace.
#[cfg(feature = "migrations")]
pub type SchemaSourceBuilder = fn() -> Result<Schema, SchemaLoadError>;

/// A schema contribution associated with a migration namespace.
#[cfg(feature = "migrations")]
#[derive(Clone)]
pub struct SchemaSource {
    namespace: Option<&'static str>,
    source: SchemaSourceKind,
}

#[cfg(feature = "migrations")]
#[derive(Clone)]
enum SchemaSourceKind {
    Builder(SchemaSourceBuilder),
    Value(Schema),
}

#[cfg(feature = "migrations")]
impl SchemaSource {
    /// Return the migration namespace this schema contribution belongs to.
    pub fn namespace(&self) -> Option<&'static str> {
        self.namespace
    }

    /// Build the schema contribution.
    pub fn build(&self) -> Result<Schema, SchemaLoadError> {
        match &self.source {
            SchemaSourceKind::Builder(build) => build(),
            SchemaSourceKind::Value(schema) => Ok(schema.clone()),
        }
    }
}

#[cfg(feature = "migrations")]
impl MigrationSource {
    /// Return the virtual Gaman namespace for this source.
    pub fn namespace(&self) -> Option<&'static str> {
        self.namespace
    }

    /// Return the embedded migration set carried by this source.
    pub fn embedded(&self) -> &'static EmbeddedMigrations {
        self.migrations
    }

    /// Return the source directory baked into `embedded_migrations!`.
    pub fn dir(&self) -> PathBuf {
        PathBuf::from(self.migrations.dir)
    }
}

/// Register the root application migration history.
#[cfg(feature = "migrations")]
pub fn root_migration(migrations: &'static EmbeddedMigrations) -> MigrationSource {
    MigrationSource {
        namespace: None,
        migrations,
    }
}

/// Register a crate migration history under a virtual namespace.
#[cfg(feature = "migrations")]
pub fn crate_migration(
    namespace: &'static str,
    migrations: &'static EmbeddedMigrations,
) -> MigrationSource {
    MigrationSource {
        namespace: Some(namespace),
        migrations,
    }
}

/// Register a root schema contribution.
#[cfg(feature = "migrations")]
pub fn root_schema(build: SchemaSourceBuilder) -> SchemaSource {
    SchemaSource {
        namespace: None,
        source: SchemaSourceKind::Builder(build),
    }
}

/// Register a crate schema contribution under a virtual namespace.
#[cfg(feature = "migrations")]
pub fn crate_schema(namespace: &'static str, build: SchemaSourceBuilder) -> SchemaSource {
    SchemaSource {
        namespace: Some(namespace),
        source: SchemaSourceKind::Builder(build),
    }
}

/// Register a prevalidated root schema contribution.
#[cfg(feature = "migrations")]
pub fn root_schema_value(schema: Schema) -> Result<SchemaSource, MigrationError> {
    validated_schema_source(None, schema)
}

/// Register a prevalidated crate schema contribution under a virtual namespace.
#[cfg(feature = "migrations")]
pub fn crate_schema_value(
    namespace: &'static str,
    schema: Schema,
) -> Result<SchemaSource, MigrationError> {
    validate_namespace(Some(namespace))?;
    validated_schema_source(Some(namespace), schema)
}

/// Errors raised while registering or executing migrations.
#[cfg(feature = "migrations")]
#[derive(Debug, Error)]
pub enum MigrationError {
    #[error("duplicate root migration source")]
    DuplicateRoot,
    #[error("duplicate migration namespace '{0}'")]
    DuplicateNamespace(String),
    #[error("invalid migration namespace '{namespace}': {reason}")]
    InvalidNamespace { namespace: String, reason: String },
    #[error("no root migration source is registered")]
    MissingRoot,
    #[error("no migration source registered for namespace '{0}'")]
    MissingNamespace(String),
    #[error("migration engine error: {0}")]
    Engine(#[from] gaman::EngineError),
    #[error("schema source {namespace} failed: {source}")]
    SchemaSource {
        namespace: String,
        #[source]
        source: SchemaLoadError,
    },
    #[error("schema merge error: {0}")]
    Schema(#[source] SchemaLoadError),
}

/// Registry for crate-owned migration sources discovered through bundles.
#[cfg(feature = "migrations")]
#[derive(Clone, Default)]
pub struct MigrationRegistry {
    root: Option<MigrationSource>,
    crates: BTreeMap<&'static str, MigrationSource>,
    root_schema: Vec<SchemaSource>,
    crate_schema: BTreeMap<&'static str, Vec<SchemaSource>>,
}

#[cfg(feature = "migrations")]
impl MigrationRegistry {
    /// Create an empty migration registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a root or crate migration source.
    pub fn register(&mut self, source: MigrationSource) -> Result<(), MigrationError> {
        validate_namespace(source.namespace)?;
        match source.namespace {
            Some(ns) => self.register_crate(ns, source),
            None => self.register_root(source),
        }
    }

    /// Register a schema contribution.
    pub fn register_schema(&mut self, source: SchemaSource) -> Result<(), MigrationError> {
        validate_namespace(source.namespace)?;
        match source.namespace {
            Some(ns) => self.crate_schema.entry(ns).or_default().push(source),
            None => self.root_schema.push(source),
        }
        Ok(())
    }

    /// Merge another registry into this one, rejecting collisions.
    pub fn merge(&mut self, other: MigrationRegistry) -> Result<(), MigrationError> {
        if let Some(source) = other.root {
            self.register(source)?;
        }
        for source in other.crates.into_values() {
            self.register(source)?;
        }
        for source in other.root_schema {
            self.register_schema(source)?;
        }
        for sources in other.crate_schema.into_values() {
            for source in sources {
                self.register_schema(source)?;
            }
        }
        Ok(())
    }

    /// Return the root migration source, if registered.
    pub fn root(&self) -> Option<MigrationSource> {
        self.root
    }

    /// Return a crate migration source by namespace.
    pub fn get(&self, namespace: &str) -> Option<MigrationSource> {
        self.crates.get(namespace).copied()
    }

    /// Iterate over crate migration sources in deterministic namespace order.
    pub fn crates(&self) -> impl Iterator<Item = (&'static str, MigrationSource)> + '_ {
        self.crates.iter().map(|(ns, source)| (*ns, *source))
    }

    /// Build the merged schema for one namespace.
    pub fn schema_for(&self, namespace: Option<&str>) -> Result<Schema, MigrationError> {
        let sources = self.schema_sources(namespace);
        merge_schema(sources)
    }

    fn register_root(&mut self, source: MigrationSource) -> Result<(), MigrationError> {
        if self.root.is_some() {
            return Err(MigrationError::DuplicateRoot);
        }
        self.root = Some(source);
        Ok(())
    }

    fn register_crate(
        &mut self,
        ns: &'static str,
        source: MigrationSource,
    ) -> Result<(), MigrationError> {
        match self.crates.entry(ns) {
            std::collections::btree_map::Entry::Occupied(_) => {
                Err(MigrationError::DuplicateNamespace(ns.to_string()))
            }
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(source);
                Ok(())
            }
        }
    }
}

#[cfg(feature = "migrations")]
impl gaman::core::MigrationStore for MigrationRegistry {
    fn load_all<'a>(
        &'a self,
    ) -> gaman::core::BoxFuture<'a, Result<Vec<gaman::Migration>, gaman::core::StoreError>> {
        Box::pin(async move { self.load_migrations().await })
    }

    fn save<'a>(
        &'a self,
        migration: &'a gaman::Migration,
    ) -> gaman::core::BoxFuture<'a, Result<(), gaman::core::StoreError>> {
        Box::pin(async move {
            let root = self.root.ok_or_else(|| gaman::core::StoreError::Save {
                id: migration.id.clone(),
                message: "cannot save a generated migration without a root migration source"
                    .to_string(),
            })?;
            if migration.id.contains('/') {
                return Err(gaman::core::StoreError::Save {
                    id: migration.id.clone(),
                    message: "generated root migration ids cannot contain namespaces".to_string(),
                });
            }
            DirectoryMigrationStore::new(root.dir())
                .save(migration)
                .await
        })
    }
}

#[cfg(feature = "migrations")]
impl MigrationRegistry {
    /// Loads and qualifies every registered migration using Gaman's store contract.
    async fn load_migrations(&self) -> Result<Vec<gaman::Migration>, gaman::core::StoreError> {
        let mut migrations = Vec::new();
        let mut ids = HashSet::new();
        if let Some(root) = self.root {
            collect_embedded(root.embedded(), None, &mut migrations, &mut ids)?;
            self.load_root_directory(root, &mut migrations, &mut ids)
                .await?;
        }
        for (namespace, source) in &self.crates {
            collect_embedded(
                source.embedded(),
                Some(namespace),
                &mut migrations,
                &mut ids,
            )?;
        }
        migrations.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(migrations)
    }

    async fn load_root_directory(
        &self,
        root: MigrationSource,
        migrations: &mut Vec<gaman::Migration>,
        ids: &mut HashSet<String>,
    ) -> Result<(), gaman::core::StoreError> {
        let disk = DirectoryMigrationStore::new(root.dir()).load_all().await?;
        for migration in disk {
            if let Some(embedded) = migrations
                .iter()
                .find(|embedded| embedded.id == migration.id)
            {
                if same_migration(embedded, &migration)? {
                    continue;
                }
                return Err(store_load_error(
                    Some(&migration.id),
                    "embedded migration content differs from its root source file",
                ));
            }
            if !ids.insert(migration.id.clone()) {
                return Err(store_load_error(
                    Some(&migration.id),
                    "duplicate root migration id",
                ));
            }
            migrations.push(migration);
        }
        Ok(())
    }
}

#[cfg(feature = "migrations")]
fn collect_embedded(
    node: &'static EmbeddedMigrations,
    namespace: Option<&str>,
    migrations: &mut Vec<gaman::Migration>,
    ids: &mut HashSet<String>,
) -> Result<(), gaman::core::StoreError> {
    for (filename, content) in node.files {
        migrations.push(parse_embedded(filename, content, namespace, ids)?);
    }
    for (child, child_node) in node.children {
        let child_namespace = qualify(namespace, child);
        collect_embedded(child_node, Some(&child_namespace), migrations, ids)?;
    }
    Ok(())
}

#[cfg(feature = "migrations")]
fn parse_embedded(
    filename: &str,
    content: &str,
    namespace: Option<&str>,
    ids: &mut HashSet<String>,
) -> Result<gaman::Migration, gaman::core::StoreError> {
    let local_id = Path::new(filename)
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            store_load_error(None, format!("invalid migration filename '{filename}'"))
        })?;
    let mut migration = gaman::Migration::from_yaml_str(content)
        .map_err(|error| store_load_error(Some(filename), error.to_string()))?;
    migration.id = qualify(namespace, local_id);
    migration.dependencies = migration
        .dependencies
        .into_iter()
        .map(|dependency| qualify_dependency(namespace, dependency))
        .collect();
    if !ids.insert(migration.id.clone()) {
        return Err(store_load_error(
            Some(&migration.id),
            "duplicate qualified migration id",
        ));
    }
    Ok(migration)
}

#[cfg(feature = "migrations")]
fn same_migration(
    left: &gaman::Migration,
    right: &gaman::Migration,
) -> Result<bool, gaman::core::StoreError> {
    let left = left
        .to_yaml_string()
        .map_err(|error| store_load_error(Some(&left.id), error.to_string()))?;
    let right = right
        .to_yaml_string()
        .map_err(|error| store_load_error(Some(&right.id), error.to_string()))?;
    Ok(left == right)
}

#[cfg(feature = "migrations")]
fn qualify(namespace: Option<&str>, id: &str) -> String {
    namespace
        .map(|namespace| format!("{namespace}/{id}"))
        .unwrap_or_else(|| id.to_string())
}

#[cfg(feature = "migrations")]
fn qualify_dependency(namespace: Option<&str>, dependency: String) -> String {
    if dependency.contains('/') {
        dependency
    } else {
        qualify(namespace, &dependency)
    }
}

#[cfg(feature = "migrations")]
fn store_load_error(id: Option<&str>, message: impl Into<String>) -> gaman::core::StoreError {
    gaman::core::StoreError::Load {
        id: id.map(str::to_owned),
        message: message.into(),
    }
}

#[cfg(feature = "migrations")]
fn merge_schema(sources: Vec<SchemaSource>) -> Result<Schema, MigrationError> {
    let mut schema = Schema::default();
    for source in sources {
        let namespace = source.namespace.unwrap_or("root").to_string();
        schema = schema
            .merge(
                source
                    .build()
                    .map_err(|source| MigrationError::SchemaSource { namespace, source })?,
            )
            .map_err(MigrationError::Schema)?;
    }
    Ok(schema)
}

#[cfg(feature = "migrations")]
impl MigrationRegistry {
    fn schema_sources(&self, namespace: Option<&str>) -> Vec<SchemaSource> {
        match namespace {
            Some(ns) => self
                .crate_schema
                .get(ns)
                .map(|items| items.to_vec())
                .unwrap_or_default(),
            None => self
                .root_schema
                .iter()
                .cloned()
                .chain(self.crate_schema.values().flatten().cloned())
                .collect(),
        }
    }
}

#[cfg(feature = "migrations")]
fn validated_schema_source(
    namespace: Option<&'static str>,
    schema: Schema,
) -> Result<SchemaSource, MigrationError> {
    schema
        .validate_checked()
        .map_err(SchemaLoadError::Validation)
        .map_err(|source| MigrationError::SchemaSource {
            namespace: namespace.unwrap_or("root").to_string(),
            source,
        })?;
    Ok(SchemaSource {
        namespace,
        source: SchemaSourceKind::Value(schema),
    })
}

#[cfg(feature = "migrations")]
fn validate_namespace(namespace: Option<&str>) -> Result<(), MigrationError> {
    let Some(namespace) = namespace else {
        return Ok(());
    };
    if namespace.is_empty() {
        return Err(invalid_ns(namespace, "namespace cannot be empty"));
    }
    if namespace.contains('/') {
        return Err(invalid_ns(namespace, "namespace cannot contain '/'"));
    }
    if !namespace
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        return Err(invalid_ns(
            namespace,
            "use lowercase letters, digits, and underscores only",
        ));
    }
    Ok(())
}

#[cfg(feature = "migrations")]
fn invalid_ns(namespace: &str, reason: &str) -> MigrationError {
    MigrationError::InvalidNamespace {
        namespace: namespace.to_string(),
        reason: reason.to_string(),
    }
}
