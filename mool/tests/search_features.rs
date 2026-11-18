#[cfg(any(feature = "text-search", feature = "fuzzy-search", feature = "vector"))]
use db::Model as _;
#[cfg(any(feature = "text-search", feature = "fuzzy-search", feature = "vector"))]
use mool as db;

#[cfg(feature = "text-search")]
#[derive(Debug, Clone, db::Model)]
#[table(full_text(
    name = "content",
    columns = ["title", "body"],
    index_name = "documents_content_search_idx",
    postgres(config = "english")
))]
struct TextDocument {
    #[column(primary_key)]
    id: i64,
    title: String,
    body: Option<String>,
}

#[cfg(all(
    feature = "text-search",
    any(feature = "postgres", feature = "mysql", feature = "mariadb")
))]
#[derive(Debug, Clone, db::Record)]
#[table(name = "text_document")]
struct TextHit {
    id: i64,
    relevance: f64,
}

#[cfg(feature = "fuzzy-search")]
#[derive(Debug, Clone, db::Model)]
struct FuzzyDocument {
    #[column(primary_key)]
    id: i64,
    #[column(fuzzy(index = "gist", index_name = "fuzzy_documents_title_trgm_idx"))]
    title: String,
}

#[cfg(all(feature = "fuzzy-search", feature = "postgres"))]
#[derive(Debug, Clone, db::Model)]
struct PlainTextDocument {
    #[column(primary_key)]
    id: i64,
    title: String,
}

#[cfg(feature = "vector")]
#[derive(Debug, Clone, db::Model)]
struct VectorDocument {
    #[column(primary_key)]
    id: i64,
    #[column(vector(
        dimensions = 3,
        metric = "cosine",
        index = "hnsw",
        index_name = "vector_documents_embedding_idx",
        postgres(m = 16, ef_construction = 64),
        mariadb(m = 8)
    ))]
    embedding: db::types::Vector,
}

#[cfg(all(feature = "vector", any(feature = "postgres", feature = "mariadb")))]
#[derive(Debug, Clone, db::Model)]
struct DynamicVectorDocument {
    #[column(primary_key)]
    id: i64,
    embedding: db::types::Vector,
}

#[cfg(all(feature = "postgres", feature = "text-search"))]
#[derive(Debug, Clone, db::Model)]
#[table(
    name = "stable_search_document",
    full_text(
        name = "old_content",
        columns = ["title"],
        index_name = "stable_search_document_fts_idx"
    )
)]
struct OldLogicalSearchDocument {
    #[column(primary_key)]
    id: i64,
    title: String,
}

#[cfg(all(feature = "postgres", feature = "text-search"))]
#[derive(Debug, Clone, db::Model)]
#[table(
    name = "stable_search_document",
    full_text(
        name = "new_content",
        columns = ["title"],
        index_name = "stable_search_document_fts_idx"
    )
)]
struct NewLogicalSearchDocument {
    #[column(primary_key)]
    id: i64,
    title: String,
}

#[cfg(all(
    feature = "postgres",
    feature = "text-search",
    feature = "fuzzy-search",
    feature = "vector"
))]
#[derive(Debug, Clone, db::Model)]
#[table(full_text(name = "content", columns = ["title"]))]
struct DefaultSearchDocument {
    #[column(primary_key)]
    id: i64,
    #[column(fuzzy(index = "gin"))]
    title: String,
    #[column(vector(dimensions = 3, metric = "cosine", index = "hnsw"))]
    embedding: db::types::Vector,
}

/// Verifies named full-text metadata resolves without repeating model columns.
#[cfg(all(
    feature = "text-search",
    any(feature = "postgres", feature = "mysql", feature = "mariadb")
))]
#[test]
fn named_full_text_search_plans_from_table_metadata() {
    let documents = TextDocument::table();
    let query = db::var::<String>().named("query");
    let relevance = documents.text_relevance("content", &query);
    let plan = db::from(&documents)
        .filter(documents.text_search("content", &query))
        .sort(relevance.desc())
        .bind(&query, "rust database".to_string())
        .all::<TextDocument>()
        .plan()
        .expect("named full-text plan");

    #[cfg(feature = "postgres")]
    assert_eq!(
        plan.sql,
        "SELECT text_document.id, text_document.title, text_document.body FROM text_document WHERE (to_tsvector('english'::regconfig, coalesce(text_document.title, '') || ' ' || coalesce(text_document.body, '')) @@ plainto_tsquery('english'::regconfig, $1)) ORDER BY CAST(ts_rank_cd(to_tsvector('english'::regconfig, coalesce(text_document.title, '') || ' ' || coalesce(text_document.body, '')), plainto_tsquery('english'::regconfig, $1)) AS double precision) DESC"
    );
    #[cfg(any(feature = "mysql", feature = "mariadb"))]
    assert_eq!(
        plan.sql,
        "SELECT text_document.id, text_document.title, text_document.body FROM text_document WHERE (MATCH (text_document.title, text_document.body) AGAINST (? IN NATURAL LANGUAGE MODE) > 0) ORDER BY MATCH (text_document.title, text_document.body) AGAINST (? IN NATURAL LANGUAGE MODE) DESC"
    );
    #[cfg(feature = "postgres")]
    assert_eq!(
        plan.params
            .get("query")
            .expect("named query parameter")
            .occurrences,
        vec![1, 1]
    );
    #[cfg(any(feature = "mysql", feature = "mariadb"))]
    assert_eq!(
        plan.params
            .get("query")
            .expect("named query parameter")
            .occurrences,
        vec![1, 2]
    );
}

/// Verifies unknown full-text names fail at the existing planning boundary.
#[cfg(feature = "text-search")]
#[test]
fn unknown_full_text_name_is_a_structured_planning_error() {
    let documents = TextDocument::table();
    let error = db::from(&documents)
        .filter(documents.text_search("missing", db::val("query".to_string())))
        .count()
        .plan()
        .expect_err("unknown search name");

    assert!(matches!(
        error,
        db::QueryError::UnknownSearchDefinition { name, .. } if name == "missing"
    ));
}

/// Verifies the shared metadata namespace distinguishes wrong capability from unknown names.
#[cfg(all(
    feature = "text-search",
    feature = "fuzzy-search",
    feature = "postgres"
))]
#[test]
fn wrong_search_capability_is_a_structured_planning_error() {
    let documents = FuzzyDocument::table();
    let error = db::from(&documents)
        .filter(documents.text_search("title", db::val("mool".to_string())))
        .count()
        .plan()
        .expect_err("fuzzy identity requested as full-text");

    assert!(matches!(
        error,
        db::QueryError::SearchCapabilityMismatch {
            requested: "full-text search",
            declared: "fuzzy search",
            ..
        }
    ));
}

/// Verifies relevance composes through Mool's ordinary output projection and slice terminal.
#[cfg(all(
    feature = "text-search",
    any(feature = "postgres", feature = "mysql", feature = "mariadb")
))]
#[test]
fn text_relevance_projects_through_the_common_expression_api() {
    let documents = TextDocument::table();
    let query = db::var::<String>().named("query");
    let output = db::out::<TextHit>();
    let relevance = documents.text_relevance("content", &query);
    let plan = db::from(&documents)
        .filter(documents.text_search("content", &query))
        .sort(relevance.clone().desc())
        .slice::<TextHit>(0, 20)
        .set(&output.id, documents.id.clone())
        .set(&output.relevance, relevance)
        .bind(&query, "rust database framework".to_string())
        .plan()
        .expect("full-text relevance projection");

    assert!(plan.sql.contains(" AS relevance"));
    assert!(plan.sql.ends_with("LIMIT 20 OFFSET 0"));
}

/// Verifies fuzzy expressions require declared column metadata and preserve exact thresholds.
#[cfg(all(feature = "fuzzy-search", feature = "postgres"))]
#[test]
fn fuzzy_similarity_and_threshold_render_as_structured_expressions() {
    use db::FuzzyTextExt as _;

    let documents = FuzzyDocument::table();
    let query = db::var::<String>().named("query");
    let plan = db::from(&documents)
        .filter(documents.title.fuzzy_match(&query, db::val(0.4_f64)))
        .sort(documents.title.fuzzy_distance(&query).asc())
        .bind(&query, "mool".to_string())
        .all::<FuzzyDocument>()
        .plan()
        .expect("fuzzy plan");
    assert_eq!(
        plan.sql,
        "SELECT fuzzy_document.id, fuzzy_document.title FROM fuzzy_document WHERE (CAST(similarity(fuzzy_document.title, $1) AS double precision) >= $2) ORDER BY (fuzzy_document.title <-> $1) ASC"
    );
}

/// Verifies fuzzy methods on an undeclared text column fail during planning.
#[cfg(all(feature = "fuzzy-search", feature = "postgres"))]
#[test]
fn fuzzy_column_requires_explicit_model_metadata() {
    use db::FuzzyTextExt as _;

    let documents = PlainTextDocument::table();
    let error = db::from(&documents)
        .filter(
            documents
                .title
                .fuzzy_match(db::val("mool".to_string()), db::val(0.4_f64)),
        )
        .count()
        .plan()
        .expect_err("undeclared fuzzy column");

    assert!(matches!(
        error,
        db::QueryError::UnknownSearchDefinition { name, .. } if name == "title"
    ));
}

/// Verifies PostgreSQL-specific parser and trigram modes remain ordinary expressions.
#[cfg(all(
    feature = "postgres",
    feature = "text-search",
    feature = "fuzzy-search"
))]
#[test]
fn postgres_search_extension_modes_render_natural_operators() {
    use db::backend::{PostgresFullTextExt as _, PostgresTextQueryMode, PostgresTrigramExt as _};

    let text = TextDocument::table();
    let phrase = db::from(&text)
        .filter(text.text_search_mode(
            "content",
            db::val("rust framework".to_string()),
            PostgresTextQueryMode::Phrase,
        ))
        .count()
        .plan()
        .expect("PostgreSQL phrase search");
    assert!(phrase.sql.contains("phraseto_tsquery('english'::regconfig"));

    let fuzzy = FuzzyDocument::table();
    let trigram = db::from(&fuzzy)
        .filter(fuzzy.title.trigram_match(db::val("mool".to_string())))
        .scalar(fuzzy.title.word_similarity(db::val("mool".to_string())))
        .plan()
        .expect("PostgreSQL trigram modes");
    assert!(trigram.sql.contains("fuzzy_document.title % $2"));
    assert!(
        trigram
            .sql
            .contains("word_similarity(fuzzy_document.title, $1)")
    );
}

/// Verifies MySQL-family boolean mode is confined to its dialect extension trait.
#[cfg(all(feature = "text-search", any(feature = "mysql", feature = "mariadb")))]
#[test]
fn mysql_family_boolean_mode_renders_match_against() {
    use db::backend::{MySqlFullTextExt as _, MySqlFullTextMode};

    let documents = TextDocument::table();
    let plan = db::from(&documents)
        .filter(documents.text_search_mode(
            "content",
            db::val("+rust -orm".to_string()),
            MySqlFullTextMode::Boolean,
        ))
        .count()
        .plan()
        .expect("MySQL-family boolean full-text search");

    assert!(plan.sql.contains("AGAINST (? IN BOOLEAN MODE)"));
}

/// Verifies dynamic vector values can be checked against schema dimensions.
#[cfg(feature = "vector")]
#[test]
fn vector_value_rejects_dimension_mismatch() {
    let vector = db::types::Vector::try_from_vec(vec![1.0, 2.0]).expect("dynamic vector");
    let error = vector
        .validate_dimensions(3)
        .expect_err("dimension mismatch");
    assert_eq!(
        error,
        db::types::VectorError::Dimension {
            expected: 3,
            actual: 2,
        }
    );
}

/// Verifies dynamic vectors reject empty and non-finite runtime values.
#[cfg(feature = "vector")]
#[test]
fn dynamic_vector_values_validate_elements() {
    assert_eq!(
        db::types::Vector::try_from_vec(Vec::new()).expect_err("empty vector"),
        db::types::VectorError::Empty
    );
    assert_eq!(
        db::types::Vector::try_from_vec(vec![1.0, f32::NAN]).expect_err("non-finite vector"),
        db::types::VectorError::NonFinite { index: 1 }
    );
}

/// Verifies portable vector distance remains a normal sortable expression.
#[cfg(any(
    all(feature = "vector", feature = "postgres"),
    all(feature = "vector", feature = "mariadb")
))]
#[test]
fn vector_cosine_distance_plans_for_supported_backends() {
    use db::VectorSimilarityExt as _;

    let documents = VectorDocument::table();
    let needle = db::var::<db::types::Vector>().named("needle");
    let plan = db::from(&documents)
        .sort(documents.embedding.cosine_distance(&needle).asc())
        .bind(&needle, db::types::Vector::from_array([1.0, 2.0, 3.0]))
        .slice::<VectorDocument>(0, 10)
        .plan()
        .expect("vector distance plan");

    #[cfg(feature = "postgres")]
    assert_eq!(
        plan.sql,
        "SELECT vector_document.id, vector_document.embedding FROM vector_document ORDER BY (vector_document.embedding <=> $1) ASC LIMIT 10 OFFSET 0"
    );
    #[cfg(feature = "mariadb")]
    assert_eq!(
        plan.sql,
        "SELECT vector_document.id, vector_document.embedding FROM vector_document ORDER BY VEC_DISTANCE_COSINE(vector_document.embedding, ?) ASC LIMIT 10 OFFSET 0"
    );
}

/// Verifies a bare Vector field infers dynamic PostgreSQL metadata without an attribute.
#[cfg(all(feature = "vector", feature = "postgres"))]
#[test]
fn vector_field_infers_dynamic_exact_search() {
    use db::VectorSimilarityExt as _;

    let documents = DynamicVectorDocument::table();
    let plan = db::from(&documents)
        .sort(
            documents
                .embedding
                .cosine_distance(db::val(db::types::Vector::from_array([1.0, 2.0])))
                .asc(),
        )
        .slice::<DynamicVectorDocument>(0, 5)
        .plan()
        .expect("dynamic vector query");
    assert!(plan.sql.contains("embedding <=> $1"));

    let schema = db::schema()
        .model::<DynamicVectorDocument>()
        .build()
        .expect("dynamic PostgreSQL vector schema");
    let table = schema
        .tables
        .get("dynamic_vector_document")
        .expect("dynamic vector table");
    assert_eq!(table.columns[1].col_type, "vector");
    assert!(table.indexes.is_empty());
}

/// Verifies MariaDB rejects unconstrained Vector storage because VECTOR(N) is required.
#[cfg(all(feature = "vector", feature = "mariadb"))]
#[test]
fn mariadb_dynamic_vector_schema_requires_dimensions() {
    let error = db::schema()
        .model::<DynamicVectorDocument>()
        .build()
        .expect_err("MariaDB requires VECTOR(N)");
    assert!(
        error
            .to_string()
            .contains("MariaDB vector column 'embedding' requires dimensions")
    );
}

/// Verifies PostgreSQL-only vector operators use pgvector's native ordering operators.
#[cfg(all(feature = "vector", feature = "postgres"))]
#[test]
fn postgres_vector_extension_operators_render_exactly() {
    use db::backend::PostgresVectorExt as _;

    let documents = VectorDocument::table();
    let needle = db::val(db::types::Vector::from_array([1.0, 2.0, 3.0]));
    let inner = db::from(&documents)
        .scalar(documents.embedding.inner_product(needle.clone()))
        .plan()
        .expect("negative inner product plan");
    let l1 = db::from(&documents)
        .scalar(documents.embedding.l1_distance(needle))
        .plan()
        .expect("L1 distance plan");

    assert_eq!(
        inner.sql,
        "SELECT (vector_document.embedding <#> $1) FROM vector_document LIMIT 1"
    );
    assert_eq!(
        l1.sql,
        "SELECT (vector_document.embedding <+> $1) FROM vector_document LIMIT 1"
    );
}

/// Verifies search model metadata creates natural PostgreSQL extensions and advanced indexes.
#[cfg(all(
    feature = "postgres",
    feature = "text-search",
    feature = "fuzzy-search",
    feature = "vector"
))]
#[test]
fn postgres_search_schema_registers_extensions_and_indexes() {
    let schema = db::schema()
        .model::<TextDocument>()
        .model::<FuzzyDocument>()
        .model::<VectorDocument>()
        .build()
        .expect("search schema");

    assert!(schema.extensions.contains_key("pg_trgm"));
    assert!(schema.extensions.contains_key("vector"));
    let text = schema.tables.get("text_document").expect("text table");
    assert_eq!(text.indexes[0].name, "documents_content_search_idx");
    assert!(
        text.indexes[0]
            .raw_sql()
            .is_some_and(|sql| sql.contains("USING GIN (to_tsvector('english'::regconfig"))
    );
    let fuzzy = schema.tables.get("fuzzy_document").expect("fuzzy table");
    assert!(
        fuzzy.indexes[0]
            .raw_sql()
            .is_some_and(|sql| sql.contains("USING gist (\"title\" gist_trgm_ops)"))
    );
    let vector = schema.tables.get("vector_document").expect("vector table");
    assert!(
        vector.indexes[0]
            .raw_sql()
            .is_some_and(|sql| sql.contains("USING hnsw (\"embedding\" vector_cosine_ops)"))
    );
}

/// Verifies default physical search index names are deterministic and capability-specific.
#[cfg(all(
    feature = "postgres",
    feature = "text-search",
    feature = "fuzzy-search",
    feature = "vector"
))]
#[test]
fn default_search_index_names_are_deterministic() {
    let schema = db::schema()
        .model::<DefaultSearchDocument>()
        .build()
        .expect("default search names schema");
    let table = schema
        .tables
        .get("default_search_document")
        .expect("default search table");
    let names = table
        .indexes
        .iter()
        .map(|index| index.name.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        names,
        vec![
            "default_search_document_content_fts_idx",
            "default_search_document_title_trgm_idx",
            "default_search_document_embedding_cosine_vec_idx",
        ]
    );
}

/// Verifies automatic and explicit PostgreSQL extension registrations deduplicate by identity.
#[cfg(all(feature = "postgres", feature = "fuzzy-search"))]
#[test]
fn automatic_and_explicit_search_extensions_are_deduplicated() {
    let schema = db::schema()
        .extension("pg_trgm")
        .model::<FuzzyDocument>()
        .build()
        .expect("deduplicated pg_trgm schema");

    assert_eq!(
        schema
            .extensions
            .keys()
            .filter(|name| name.as_str() == "pg_trgm")
            .count(),
        1
    );
}

/// Verifies changing only a logical FTS name preserves an explicitly stable physical index.
#[cfg(all(feature = "postgres", feature = "text-search"))]
#[test]
fn logical_search_name_is_independent_from_physical_index_identity() {
    let old = db::schema()
        .model::<OldLogicalSearchDocument>()
        .build()
        .expect("old logical search schema");
    let new = db::schema()
        .model::<NewLogicalSearchDocument>()
        .build()
        .expect("new logical search schema");
    let operations = gaman_core::diff::DiffEngine::new()
        .diff(&new, &old, &gaman_core::dialects::Dialect::Postgres)
        .expect("logical-only search rename diff");

    assert!(operations.is_empty());
    let old_table = OldLogicalSearchDocument::table();
    let old_query = db::from(&old_table)
        .filter(old_table.text_search("old_content", db::val("mool".to_string())))
        .count()
        .plan()
        .expect("old logical name");
    let new_table = NewLogicalSearchDocument::table();
    let new_query = db::from(&new_table)
        .filter(new_table.text_search("new_content", db::val("mool".to_string())))
        .count()
        .plan()
        .expect("new logical name");
    assert_eq!(old_query.sql, new_query.sql);
}

/// Verifies manual metadata builders receive defensive duplicate and identifier validation.
#[cfg(all(feature = "postgres", feature = "text-search"))]
#[test]
fn manual_search_metadata_is_validated_before_schema_output() {
    let duplicate = db::search::SearchTableMetadata::new()
        .full_text(db::search::FullTextDefinition::new("content", ["title"]))
        .full_text(db::search::FullTextDefinition::new("content", ["body"]));
    let duplicate_error =
        db::search::__private::validate_dialect(&duplicate, db::gaman::core::Dialect::Postgres)
            .expect_err("duplicate manual search metadata");
    assert!(duplicate_error.contains("duplicate search definition 'content'"));

    let invalid = db::search::SearchTableMetadata::new().full_text(
        db::search::FullTextDefinition::new("content", ["title"])
            .postgres_config("english; DROP TABLE documents"),
    );
    let invalid_error =
        db::search::__private::validate_dialect(&invalid, db::gaman::core::Dialect::Postgres)
            .expect_err("invalid manual PostgreSQL configuration");
    assert!(invalid_error.contains("invalid SQL identifier"));
}

/// Verifies migration planning orders extensions before tables and advanced indexes.
#[cfg(all(
    feature = "postgres",
    feature = "text-search",
    feature = "fuzzy-search",
    feature = "vector"
))]
#[test]
fn postgres_search_migration_dependencies_are_ordered_and_reversible() {
    use gaman_core::diff::DiffEngine;
    use gaman_core::schema::{Operation, Schema};

    let desired = db::schema()
        .model::<TextDocument>()
        .model::<FuzzyDocument>()
        .model::<VectorDocument>()
        .build()
        .expect("search schema");
    let dialect = gaman_core::dialects::Dialect::Postgres;
    let apply = DiffEngine::new()
        .diff(&desired, &Schema::default(), &dialect)
        .expect("blank to search schema diff");
    let first_table = operation_position(&apply, |operation| {
        matches!(operation, Operation::CreateTable { .. })
    });
    let last_extension = operation_rposition(&apply, |operation| {
        matches!(operation, Operation::CreateExtension { .. })
    });
    assert!(last_extension < first_table);
    assert!(apply.iter().any(|operation| matches!(
        operation,
        Operation::CreateTable { table } if table.indexes.iter().any(|index| index.is_opaque())
    )));

    let remove = DiffEngine::new()
        .diff(&Schema::default(), &desired, &dialect)
        .expect("search schema removal diff");
    for table_name in ["text_document", "fuzzy_document", "vector_document"] {
        let index = operation_position(
            &remove,
            |operation| matches!(operation, Operation::DropIndex { table_name: owner, .. } if owner == table_name),
        );
        let table = operation_position(
            &remove,
            |operation| matches!(operation, Operation::DropTable { table } if table.name == table_name),
        );
        assert!(index < table);
    }
    let last_table = operation_rposition(&remove, |operation| {
        matches!(operation, Operation::DropTable { .. })
    });
    let first_extension = operation_position(&remove, |operation| {
        matches!(operation, Operation::DropExtension { .. })
    });
    assert!(last_table < first_extension);
}

/// Verifies model-to-table conversion excludes schema-level advanced search indexes.
#[cfg(all(feature = "postgres", feature = "text-search"))]
#[test]
fn into_table_keeps_search_indexes_out_of_modeled_state() {
    let table = <TextDocument as mool::schema::IntoTable>::into_table(
        &gaman_core::dialects::Dialect::Postgres,
    );

    assert!(table.indexes.is_empty());
}

/// Verifies a changed opaque search definition produces a conservative drop and recreate.
#[cfg(all(feature = "postgres", feature = "text-search"))]
#[test]
fn changed_search_index_definition_drops_and_recreates() {
    use gaman_core::diff::DiffEngine;
    use gaman_core::schema::Operation;

    let previous = db::schema()
        .model::<TextDocument>()
        .build()
        .expect("previous search schema");
    let desired = db::schema()
        .table::<TextDocument>()
        .opaque(
            "CREATE INDEX \"documents_content_search_idx\" ON \"text_document\" USING GIN (to_tsvector('simple'::regconfig, coalesce(\"title\", '')))",
        )
        .build()
        .expect("changed search schema");
    let operations = DiffEngine::new()
        .diff(
            &desired,
            &previous,
            &gaman_core::dialects::Dialect::Postgres,
        )
        .expect("changed search index diff");

    assert!(matches!(
        operations.first(),
        Some(Operation::DropIndex { .. })
    ));
    assert!(matches!(
        operations.last(),
        Some(Operation::AddIndex { .. })
    ));
}

#[cfg(all(
    feature = "postgres",
    feature = "text-search",
    feature = "fuzzy-search",
    feature = "vector"
))]
fn operation_position(
    operations: &[gaman_core::schema::Operation],
    predicate: impl Fn(&gaman_core::schema::Operation) -> bool,
) -> usize {
    operations
        .iter()
        .position(predicate)
        .expect("expected migration operation")
}

#[cfg(all(
    feature = "postgres",
    feature = "text-search",
    feature = "fuzzy-search",
    feature = "vector"
))]
fn operation_rposition(
    operations: &[gaman_core::schema::Operation],
    predicate: impl Fn(&gaman_core::schema::Operation) -> bool,
) -> usize {
    operations
        .iter()
        .rposition(predicate)
        .expect("expected migration operation")
}

/// Verifies MySQL-family full-text schemas use a multi-column FULLTEXT index.
#[cfg(all(feature = "text-search", any(feature = "mysql", feature = "mariadb")))]
#[test]
fn mysql_family_schema_uses_a_natural_fulltext_index() {
    let schema = db::schema()
        .model::<TextDocument>()
        .build()
        .expect("MySQL-family full-text schema");
    let table = schema.tables.get("text_document").expect("text table");

    assert_eq!(table.indexes[0].name, "documents_content_search_idx");
    assert_eq!(
        table.indexes[0].raw_sql(),
        Some(
            "CREATE FULLTEXT INDEX `documents_content_search_idx` ON `text_document` (`title`, `body`)"
        )
    );
}

/// Verifies MariaDB vector schema uses VECTOR storage and its native vector index syntax.
#[cfg(all(feature = "vector", feature = "mariadb"))]
#[test]
fn mariadb_schema_uses_vector_storage_and_index() {
    let schema = db::schema()
        .model::<VectorDocument>()
        .build()
        .expect("MariaDB vector schema");
    let table = schema.tables.get("vector_document").expect("vector table");

    assert_eq!(table.columns[1].col_type, "VECTOR(3)");
    assert_eq!(
        table.indexes[0].raw_sql(),
        Some(
            "CREATE VECTOR INDEX `vector_documents_embedding_idx` ON `vector_document` (`embedding`) M=8 DISTANCE=COSINE"
        )
    );
}

/// Verifies SQLite rejects otherwise valid search metadata before producing executable SQL.
#[cfg(all(
    feature = "sqlite",
    feature = "text-search",
    feature = "fuzzy-search",
    feature = "vector"
))]
#[test]
fn sqlite_rejects_search_plans_and_schema_metadata() {
    use db::{FuzzyTextExt as _, VectorSimilarityExt as _};

    let text = TextDocument::table();
    let full_text = db::from(&text)
        .filter(text.text_search("content", db::val("mool".to_string())))
        .count()
        .plan()
        .expect_err("SQLite full-text search");
    assert!(matches!(full_text, db::QueryError::Unsupported { .. }));

    let fuzzy = FuzzyDocument::table();
    let fuzzy_error = db::from(&fuzzy)
        .scalar(fuzzy.title.fuzzy_similarity(db::val("mool".to_string())))
        .plan()
        .expect_err("SQLite fuzzy search");
    assert!(matches!(fuzzy_error, db::QueryError::Unsupported { .. }));

    let vector = VectorDocument::table();
    let vector_error = db::from(&vector)
        .scalar(
            vector
                .embedding
                .cosine_distance(db::val(db::types::Vector::from_array([1.0, 2.0, 3.0]))),
        )
        .plan()
        .expect_err("SQLite vector search");
    assert!(matches!(vector_error, db::QueryError::Unsupported { .. }));

    assert!(db::schema().model::<TextDocument>().build().is_err());
    assert!(db::schema().model::<FuzzyDocument>().build().is_err());
    assert!(db::schema().model::<VectorDocument>().build().is_err());
}
