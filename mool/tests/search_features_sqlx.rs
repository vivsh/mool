#![cfg(any(feature = "postgres", feature = "mysql", feature = "mariadb"))]

#[cfg(any(feature = "text-search", feature = "fuzzy-search", feature = "vector"))]
use db::{DbSession as _, Model as _};
#[cfg(any(feature = "text-search", feature = "fuzzy-search", feature = "vector"))]
use mool as db;

#[cfg(all(feature = "postgres", feature = "text-search"))]
#[derive(Debug, Clone, PartialEq, db::Model)]
#[table(
    name = "mool_text_documents",
    full_text(
        name = "content",
        columns = ["title", "body"],
        postgres(config = "english")
    )
)]
struct PostgresTextDocument {
    #[column(primary_key)]
    id: i64,
    title: String,
    body: Option<String>,
}

#[cfg(all(feature = "postgres", feature = "fuzzy-search"))]
#[derive(Debug, Clone, PartialEq, db::Model)]
#[table(name = "mool_fuzzy_documents")]
struct PostgresFuzzyDocument {
    #[column(primary_key)]
    id: i64,
    #[column(fuzzy(index = "gist"))]
    title: String,
}

#[cfg(all(feature = "postgres", feature = "vector"))]
#[derive(Debug, Clone, PartialEq, db::Model)]
#[table(name = "mool_vector_documents")]
struct PostgresVectorDocument {
    #[column(primary_key)]
    id: i64,
    #[column(vector(dimensions = 3, metric = "cosine", index = "hnsw"))]
    embedding: db::types::Vector,
}

#[cfg(all(feature = "postgres", feature = "vector"))]
#[derive(Debug, Clone, PartialEq, db::Model)]
#[table(name = "mool_dynamic_vector_documents")]
struct PostgresDynamicVectorDocument {
    #[column(primary_key)]
    id: i64,
    embedding: db::types::Vector,
}

#[cfg(all(
    feature = "postgres",
    feature = "text-search",
    feature = "fuzzy-search",
    feature = "vector"
))]
#[derive(Debug, Clone, PartialEq, db::Model)]
#[table(
    name = "mool_search_documents",
    full_text(
        name = "content",
        columns = ["title", "body"],
        postgres(config = "english")
    )
)]
struct PostgresSearchDocument {
    #[column(primary_key)]
    id: i64,
    #[column(fuzzy(index = "gist"))]
    title: String,
    body: Option<String>,
    #[column(vector(dimensions = 3, metric = "cosine", index = "hnsw"))]
    embedding: db::types::Vector,
}

#[cfg(all(feature = "text-search", any(feature = "mysql", feature = "mariadb")))]
#[derive(Debug, Clone, PartialEq, db::Model)]
#[table(
    name = "mool_search_documents",
    full_text(name = "content", columns = ["title", "body"])
)]
struct MySqlSearchDocument {
    #[column(primary_key)]
    id: i64,
    title: String,
    body: Option<String>,
}

#[cfg(all(feature = "mariadb", feature = "vector"))]
#[derive(Debug, Clone, PartialEq, db::Model)]
#[table(name = "mool_vector_documents")]
struct MariaDbVectorDocument {
    #[column(primary_key)]
    id: i64,
    #[column(vector(dimensions = 3, metric = "cosine", index = "hnsw", mariadb(m = 8)))]
    embedding: db::types::Vector,
}

#[cfg(all(feature = "postgres", feature = "vector"))]
#[derive(Debug, db::sqlx::FromRow)]
struct ExplainRow {
    #[sqlx(rename = "QUERY PLAN")]
    plan: String,
}

/// Verifies PostgreSQL full-text matching and relevance without optional extensions.
#[cfg(all(feature = "postgres", feature = "text-search"))]
#[db::sqlx::test]
#[ignore = "requires PostgreSQL full-text facilities"]
async fn postgres_full_text_lifecycle(pool: db::backend::Pool) {
    let mut pool = db::DbPool::from_pool(pool);
    for sql in [
        "CREATE TABLE mool_text_documents (id BIGINT PRIMARY KEY, title TEXT NOT NULL, body TEXT)",
        "CREATE INDEX mool_text_documents_content_fts_idx ON mool_text_documents USING GIN (to_tsvector('english'::regconfig, coalesce(title, '') || ' ' || coalesce(body, '')))",
        "INSERT INTO mool_text_documents VALUES (1, 'Rust database framework', 'Typed SQL and migrations'), (2, 'Garden notes', NULL)",
    ] {
        pool.execute(statement(sql))
            .await
            .expect("PostgreSQL full-text fixture DDL");
    }
    let documents = PostgresTextDocument::table();
    let rows = db::from(&documents)
        .filter(documents.text_search("content", db::val("rust".to_string())))
        .sort(
            documents
                .text_relevance("content", db::val("rust".to_string()))
                .desc(),
        )
        .all::<PostgresTextDocument>()
        .exec(&mut pool)
        .await
        .expect("live PostgreSQL full-text query");
    assert_eq!(rows.first().map(|row| row.id), Some(1));
}

/// Verifies PostgreSQL pg_trgm matching independently from pgvector.
#[cfg(all(feature = "postgres", feature = "fuzzy-search"))]
#[db::sqlx::test]
#[ignore = "requires PostgreSQL with pg_trgm"]
async fn postgres_fuzzy_lifecycle(pool: db::backend::Pool) {
    use db::FuzzyTextExt as _;

    let mut pool = db::DbPool::from_pool(pool);
    for sql in [
        "CREATE EXTENSION IF NOT EXISTS pg_trgm",
        "CREATE TABLE mool_fuzzy_documents (id BIGINT PRIMARY KEY, title TEXT NOT NULL)",
        "CREATE INDEX mool_fuzzy_documents_title_trgm_idx ON mool_fuzzy_documents USING gist (title gist_trgm_ops)",
        "INSERT INTO mool_fuzzy_documents VALUES (1, 'Rust database framework'), (2, 'Garden notes')",
    ] {
        pool.execute(statement(sql))
            .await
            .expect("PostgreSQL fuzzy fixture DDL");
    }
    let documents = PostgresFuzzyDocument::table();
    let rows = db::from(&documents)
        .filter(
            documents
                .title
                .fuzzy_match(db::val("Rust databse".to_string()), db::val(0.3_f64)),
        )
        .all::<PostgresFuzzyDocument>()
        .exec(&mut pool)
        .await
        .expect("live PostgreSQL trigram query");
    assert_eq!(rows.first().map(|row| row.id), Some(1));
}

/// Verifies PostgreSQL pgvector CRUD, exact distance, decoding, and HNSW plan use.
#[cfg(all(feature = "postgres", feature = "vector"))]
#[db::sqlx::test]
#[ignore = "requires PostgreSQL with the vector extension"]
async fn postgres_vector_lifecycle(pool: db::backend::Pool) {
    let mut pool = db::DbPool::from_pool(pool);
    setup_postgres_vector(&mut pool).await;
    insert_postgres_vector(&mut pool).await;
    assert_postgres_vector_distance(&mut pool).await;
    assert_postgres_vector_plan(&mut pool).await;
    assert_postgres_dynamic_vectors(&mut pool).await;
}

#[cfg(all(feature = "postgres", feature = "vector"))]
async fn setup_postgres_vector(pool: &mut db::DbPool) {
    for sql in [
        "CREATE EXTENSION IF NOT EXISTS vector",
        "CREATE TABLE mool_vector_documents (id BIGINT PRIMARY KEY, embedding vector(3) NOT NULL)",
        "CREATE INDEX mool_vector_documents_embedding_vec_idx ON mool_vector_documents USING hnsw (embedding vector_cosine_ops)",
    ] {
        pool.execute(statement(sql))
            .await
            .expect("PostgreSQL vector fixture DDL");
    }
}

#[cfg(all(feature = "postgres", feature = "vector"))]
async fn insert_postgres_vector(pool: &mut db::DbPool) {
    let documents = PostgresVectorDocument::table();
    let row = PostgresVectorDocument {
        id: 1,
        embedding: db::types::Vector::from_array([1.0, 0.0, 0.0]),
    };
    db::from(&documents)
        .insert(&row)
        .exec(pool)
        .await
        .expect("insert PostgreSQL vector row");
}

#[cfg(all(feature = "postgres", feature = "vector"))]
async fn assert_postgres_vector_distance(pool: &mut db::DbPool) {
    use db::VectorSimilarityExt as _;

    let documents = PostgresVectorDocument::table();
    let decoded = db::from(&documents)
        .first::<PostgresVectorDocument>()
        .exec(&mut *pool)
        .await
        .expect("decode PostgreSQL vector row")
        .expect("PostgreSQL vector row exists");
    assert_eq!(decoded.embedding.as_slice(), &[1.0, 0.0, 0.0]);
    let distance = db::from(&documents)
        .scalar(
            documents
                .embedding
                .cosine_distance(db::val(decoded.embedding)),
        )
        .exec(pool)
        .await
        .expect("live PostgreSQL cosine distance");
    assert!(distance.abs() < f64::EPSILON);
}

#[cfg(all(feature = "postgres", feature = "vector"))]
async fn assert_postgres_vector_plan(pool: &mut db::DbPool) {
    db::sqlx::query("SET enable_seqscan = off")
        .execute(pool.as_sqlx())
        .await
        .expect("disable sequential scans for index evidence");
    let rows = db::sqlx::query_as::<_, ExplainRow>(
        "EXPLAIN SELECT id FROM mool_vector_documents ORDER BY embedding <=> '[1,0,0]' LIMIT 1",
    )
    .fetch_all(pool.as_sqlx())
    .await
    .expect("PostgreSQL vector EXPLAIN");
    assert!(
        rows.iter()
            .any(|row| row.plan.contains("mool_vector_documents_embedding_vec_idx"))
    );
}

/// Verifies one unconstrained PostgreSQL column stores multiple vector dimensions.
#[cfg(all(feature = "postgres", feature = "vector"))]
async fn assert_postgres_dynamic_vectors(pool: &mut db::DbPool) {
    pool.execute(statement(
        "CREATE TABLE mool_dynamic_vector_documents (id BIGINT PRIMARY KEY, embedding vector NOT NULL)",
    ))
    .await
    .expect("PostgreSQL dynamic vector table");
    let documents = PostgresDynamicVectorDocument::table();
    for row in [
        PostgresDynamicVectorDocument {
            id: 1,
            embedding: db::types::Vector::from_array([1.0, 2.0]),
        },
        PostgresDynamicVectorDocument {
            id: 2,
            embedding: db::types::Vector::from_array([1.0, 2.0, 3.0]),
        },
    ] {
        db::from(&documents)
            .insert(&row)
            .exec(&mut *pool)
            .await
            .expect("insert dynamic PostgreSQL vector");
    }
    let rows = db::from(&documents)
        .sort(documents.id.asc())
        .all::<PostgresDynamicVectorDocument>()
        .exec(pool)
        .await
        .expect("decode dynamic PostgreSQL vectors");
    assert_eq!(rows[0].embedding.dimensions(), 2);
    assert_eq!(rows[1].embedding.dimensions(), 3);
}

/// Verifies live PostgreSQL FTS, pg_trgm, pgvector codecs/distances, and HNSW plan use.
#[cfg(all(
    feature = "postgres",
    feature = "text-search",
    feature = "fuzzy-search",
    feature = "vector"
))]
#[db::sqlx::test]
#[ignore = "requires PostgreSQL with pg_trgm and vector extensions"]
async fn postgres_search_lifecycle(pool: db::backend::Pool) {
    let mut pool = db::DbPool::from_pool(pool);
    setup_postgres(&mut pool).await;
    insert_postgres_documents(&mut pool).await;
    assert_postgres_text_and_fuzzy(&mut pool).await;
    assert_postgres_vectors(&mut pool).await;
    assert_postgres_hnsw_plan(&mut pool).await;
}

#[cfg(all(
    feature = "postgres",
    feature = "text-search",
    feature = "fuzzy-search",
    feature = "vector"
))]
async fn setup_postgres(pool: &mut db::DbPool) {
    for sql in [
        "CREATE EXTENSION IF NOT EXISTS pg_trgm",
        "CREATE EXTENSION IF NOT EXISTS vector",
        "CREATE TABLE mool_search_documents (id BIGINT PRIMARY KEY, title TEXT NOT NULL, body TEXT, embedding vector(3) NOT NULL)",
        "CREATE INDEX mool_search_documents_content_fts_idx ON mool_search_documents USING GIN (to_tsvector('english'::regconfig, coalesce(title, '') || ' ' || coalesce(body, '')))",
        "CREATE INDEX mool_search_documents_title_trgm_idx ON mool_search_documents USING gist (title gist_trgm_ops)",
        "CREATE INDEX mool_search_documents_embedding_vec_idx ON mool_search_documents USING hnsw (embedding vector_cosine_ops)",
    ] {
        pool.execute(statement(sql))
            .await
            .expect("PostgreSQL search DDL");
    }
}

#[cfg(all(
    feature = "postgres",
    feature = "text-search",
    feature = "fuzzy-search",
    feature = "vector"
))]
async fn insert_postgres_documents(pool: &mut db::DbPool) {
    let documents = PostgresSearchDocument::table();
    for row in [
        PostgresSearchDocument {
            id: 1,
            title: "Rust database framework".to_string(),
            body: Some("Typed SQL and migrations".to_string()),
            embedding: db::types::Vector::from_array([1.0, 0.0, 0.0]),
        },
        PostgresSearchDocument {
            id: 2,
            title: "Garden notes".to_string(),
            body: None,
            embedding: db::types::Vector::from_array([0.0, 1.0, 0.0]),
        },
    ] {
        db::from(&documents)
            .insert(&row)
            .exec(&mut *pool)
            .await
            .expect("insert PostgreSQL search row");
    }
}

#[cfg(all(
    feature = "postgres",
    feature = "text-search",
    feature = "fuzzy-search",
    feature = "vector"
))]
async fn assert_postgres_text_and_fuzzy(pool: &mut db::DbPool) {
    use db::FuzzyTextExt as _;

    let documents = PostgresSearchDocument::table();
    let text = db::from(&documents)
        .filter(documents.text_search("content", db::val("rust".to_string())))
        .all::<PostgresSearchDocument>()
        .exec(&mut *pool)
        .await
        .expect("live PostgreSQL full-text query");
    assert_eq!(text.iter().map(|row| row.id).collect::<Vec<_>>(), vec![1]);

    let fuzzy = db::from(&documents)
        .filter(
            documents
                .title
                .fuzzy_match(db::val("Rust databse".to_string()), db::val(0.3_f64)),
        )
        .all::<PostgresSearchDocument>()
        .exec(&mut *pool)
        .await
        .expect("live PostgreSQL trigram query");
    assert_eq!(fuzzy.first().map(|row| row.id), Some(1));
}

#[cfg(all(
    feature = "postgres",
    feature = "text-search",
    feature = "fuzzy-search",
    feature = "vector"
))]
async fn assert_postgres_vectors(pool: &mut db::DbPool) {
    use db::VectorSimilarityExt as _;

    let documents = PostgresSearchDocument::table();
    let needle = db::val(db::types::Vector::from_array([1.0, 0.0, 0.0]));
    let decoded = db::from(&documents)
        .filter(documents.id.eq(db::val(1_i64)))
        .first::<PostgresSearchDocument>()
        .exec(&mut *pool)
        .await
        .expect("decode PostgreSQL vector row")
        .expect("PostgreSQL vector row exists");
    assert_eq!(decoded.embedding.as_slice(), &[1.0, 0.0, 0.0]);
    let distance = db::from(&documents)
        .filter(documents.id.eq(db::val(1_i64)))
        .scalar(documents.embedding.cosine_distance(needle))
        .exec(&mut *pool)
        .await
        .expect("live PostgreSQL cosine distance");
    assert!(distance.abs() < f64::EPSILON);
}

#[cfg(all(
    feature = "postgres",
    feature = "text-search",
    feature = "fuzzy-search",
    feature = "vector"
))]
async fn assert_postgres_hnsw_plan(pool: &mut db::DbPool) {
    db::sqlx::query("SET enable_seqscan = off")
        .execute(pool.as_sqlx())
        .await
        .expect("disable sequential scans for index evidence");
    let rows = db::sqlx::query_as::<_, ExplainRow>(
        "EXPLAIN SELECT id FROM mool_search_documents ORDER BY embedding <=> '[1,0,0]' LIMIT 1",
    )
    .fetch_all(pool.as_sqlx())
    .await
    .expect("PostgreSQL vector EXPLAIN");
    assert!(
        rows.iter()
            .any(|row| row.plan.contains("mool_search_documents_embedding_vec_idx"))
    );
}

/// Verifies live MySQL-family multi-column FULLTEXT matching and ranking.
#[cfg(all(feature = "text-search", any(feature = "mysql", feature = "mariadb")))]
#[db::sqlx::test]
#[ignore = "requires a MySQL-family server with FULLTEXT support"]
async fn mysql_family_full_text_lifecycle(pool: db::backend::Pool) {
    let mut pool = db::DbPool::from_pool(pool);
    for sql in [
        "CREATE TABLE mool_search_documents (id BIGINT PRIMARY KEY, title TEXT NOT NULL, body TEXT)",
        "CREATE FULLTEXT INDEX mool_search_documents_content_fts_idx ON mool_search_documents (title, body)",
        "INSERT INTO mool_search_documents (id, title, body) VALUES (1, 'Rust database framework', 'Typed SQL and migrations'), (2, 'Garden notes', NULL)",
    ] {
        pool.execute(statement(sql))
            .await
            .expect("MySQL-family search fixture DDL");
    }
    let documents = MySqlSearchDocument::table();
    let rows = db::from(&documents)
        .filter(documents.text_search("content", db::val("rust".to_string())))
        .sort(
            documents
                .text_relevance("content", db::val("rust".to_string()))
                .desc(),
        )
        .all::<MySqlSearchDocument>()
        .exec(&mut pool)
        .await
        .expect("live MySQL-family full-text query");

    assert_eq!(rows.first().map(|row| row.id), Some(1));
}

/// Verifies MariaDB 11.8 native vector CRUD, binary decoding, and exact distances.
#[cfg(all(feature = "mariadb", feature = "vector"))]
#[db::sqlx::test]
#[ignore = "requires MariaDB 11.8 vector facilities"]
async fn mariadb_vector_lifecycle(pool: db::backend::Pool) {
    use db::VectorSimilarityExt as _;

    let mut pool = db::DbPool::from_pool(pool);
    for sql in [
        "CREATE TABLE mool_vector_documents (id BIGINT PRIMARY KEY, embedding VECTOR(3) NOT NULL)",
        "CREATE VECTOR INDEX mool_vector_documents_embedding_vec_idx ON mool_vector_documents (embedding) M=8 DISTANCE=COSINE",
    ] {
        pool.execute(statement(sql))
            .await
            .expect("MariaDB vector fixture DDL");
    }
    let documents = MariaDbVectorDocument::table();
    let row = MariaDbVectorDocument {
        id: 1,
        embedding: db::types::Vector::from_array([1.0, 0.0, 0.0]),
    };
    db::from(&documents)
        .insert(&row)
        .exec(&mut pool)
        .await
        .expect("insert MariaDB vector row");
    let decoded = db::from(&documents)
        .first::<MariaDbVectorDocument>()
        .exec(&mut pool)
        .await
        .expect("decode MariaDB vector row")
        .expect("MariaDB vector row exists");
    assert_eq!(decoded.embedding.as_slice(), &[1.0, 0.0, 0.0]);
    let distance = db::from(&documents)
        .scalar(
            documents
                .embedding
                .cosine_distance(db::val(db::types::Vector::from_array([1.0, 0.0, 0.0]))),
        )
        .exec(&mut pool)
        .await
        .expect("MariaDB cosine distance");
    assert!(distance.abs() < f64::EPSILON);
}

#[cfg(any(feature = "text-search", feature = "fuzzy-search", feature = "vector"))]
fn statement(sql: &str) -> db::Statement {
    db::query(sql)
        .to_statement()
        .expect("static search fixture SQL has no bindings")
}
