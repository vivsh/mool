# AI And Search

Mool exposes three independent capabilities:

```toml
mool = { version = "0.2", features = ["postgres", "text-search"] }
mool = { version = "0.2", features = ["postgres", "fuzzy-search"] }
mool = { version = "0.2", features = ["postgres", "vector"] }
```

Select one backend feature and test each backend separately. Backend features
are mutually exclusive, so do not use `--all-features`.

## Full-Text Search

Declare a logical search name over existing text columns:

```rust
#[derive(Debug, Clone, db::Model)]
#[table(full_text(
    name = "content",
    columns = ["title", "body"],
    postgres(config = "english")
))]
struct Document {
    #[column(primary_key)]
    id: i64,
    title: String,
    body: Option<String>,
}

let documents = Document::table();
let query = db::var::<String>().named("query");

let results = db::from(&documents)
    .filter(documents.text_search("content", &query))
    .sort(documents.text_relevance("content", &query).desc())
    .bind(&query, "rust database framework".to_string())
    .slice::<Document>(0, 20);
```

The logical name is query metadata, not the physical index name. Use
`index_name = "..."` only when migration output needs a stable override.
PostgreSQL uses a GIN expression index. MySQL and MariaDB use one multi-column
`FULLTEXT` index. Mool does not add a `tsvector` column or any generated Rust
search member.

## Fuzzy Text

PostgreSQL trigram operations require an explicit non-null text declaration:

```rust
#[column(fuzzy(index = "gist"))]
title: String,

let predicate = documents
    .title
    .fuzzy_match(db::val("databse".to_string()), db::val(0.35_f64));
```

`fuzzy_match` uses an explicit threshold and does not depend on session state.
GiST is the appropriate opt-in when nearest-neighbour trigram ordering matters;
GIN serves indexed similarity predicates. PostgreSQL's `%` and word-similarity
operations are available only through `backend::postgres::PostgresTrigramExt`.
The schema registers `pg_trgm` before dependent indexes. Migration credentials
therefore need permission to create the extension, or an operator must install
it first.

## Vector Similarity

The `Vector` field type enables exact vector search without requiring an
attribute. PostgreSQL maps this minimal form to a dimension-unconstrained
`vector` column:

```rust
embedding: db::types::Vector,
```

Declare dimensions when the physical schema or an ANN index requires them:

```rust
#[column(vector(dimensions = 1536, metric = "cosine", index = "none"))]
embedding: db::types::Vector,

let needle = db::var::<db::types::Vector>().named("needle");
let nearest = db::from(&documents)
    .sort(documents.embedding.cosine_distance(&needle).asc())
    .bind(&needle, embedding)
    .slice::<Document>(0, 20);
```

`index = "none"` preserves exact search. HNSW and IVFFlat are approximate:
they can reduce recall, consume substantial memory and disk, and hold locks or
increase load while building. Add an ANN index only with workload-specific
recall/latency measurements. Verify index selection with `EXPLAIN`; successful
query execution does not demonstrate index use.

PostgreSQL requires the SQL extension named `vector` (the project is called
pgvector). It can store different dimensions in an unconstrained column, but
each distance comparison still requires equal dimensions. A conventional ANN
index requires `dimensions = N`; mixed-dimension partial expression indexes are
outside the portable API. MariaDB requires `dimensions = N`, uses its native
binary `f32` representation, and exposes native distance functions. MySQL 8.x
and SQLite vector planning is intentionally unsupported. Nullable, half,
sparse, binary, and quantized vectors are outside the initial contract.

## Operational Evidence

The implementation has deterministic compile and SQL/schema conformance tests.
Release support claims remain conditional on live fixtures for the documented
database and extension versions. Live fixtures must cover CRUD/codecs,
matching/ranking or exact distances, extension-before-DDL migration ordering,
round-trip inspection with no diff, negative privilege/version cases, and
`EXPLAIN` evidence for indexed nearest-neighbour queries.
