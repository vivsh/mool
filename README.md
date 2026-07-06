# Mool

[![Crates.io](https://img.shields.io/crates/v/mool.svg)](https://crates.io/crates/mool)
[![Docs.rs](https://docs.rs/mool/badge.svg)](https://docs.rs/mool)
[![License](https://img.shields.io/crates/l/mool.svg)](https://github.com/vivsh/mool)

Mool, pronounced `mool`, means root or source.

Mool is a source-first typed SQL data mapper for Rust.

It gives SQLx projects typed model metadata, row mapping, query handles,
relations, filters, schema metadata, migrations, test mocks, and enum mapping
without turning records into active-record objects. SQL remains visible at the
call site; Mool just makes the shape typed.

## Why Mool?

Use Mool when plain SQLx starts repeating the same database plumbing:

- derive table metadata once, then reuse typed columns everywhere
- compose selects, writes, filters, joins, subqueries, and CTEs with checked
  column/value types
- scan rows into models, projections, joined records, and write payloads
- keep SQL rendering explicit enough to inspect, test, and reason about
- share one session abstraction across pools, transactions, raw SQL, and mocks
- keep schema/migration metadata next to Rust models when that is useful

Mool is best described as a typed SQL data mapper. It is ORM-like, but not an
active-record ORM: records do not save themselves, no runtime identity map owns
your data, and queries are still written as explicit operations.

## Why Not Plain SQL?

Plain SQL is still the right tool for one-off queries, highly tuned hand-written
SQL, or database-specific statements that should stay exact.

Mool earns its keep when the same tables appear across many query paths. It
removes stringly-typed column names, repeated bind ordering, manual row scanning,
ad hoc filter builders, and test-only database setup while keeping escape hatches:

```rust
db::query("SELECT COUNT(*) FROM posts WHERE author_id = :author_id")
    .bind("author_id", 42_i64)
    .scalar::<i64>(session)
    .await?;
```

## Install

Enable exactly one backend feature for real use:

```toml
mool = { version = "0.1", features = ["postgres"] }
# or
mool = { version = "0.1", features = ["sqlite"] }
# or
mool = { version = "0.1", features = ["mysql"] }
```

Optional features:

```toml
mool = { version = "0.1", features = ["postgres", "migrations"] }
mool = { version = "0.1", features = ["sqlite", "migrations"] }
mool = { version = "0.1", features = ["sqlite", "mock"] }
```

Backend features are mutually exclusive. Do not verify with `--all-features`.
Migrations are supported for Postgres and SQLite3, not MySQL. Mock support is
available in Mool debug/test builds and behind `mock` for downstream release
builds.

## Quick Example

```rust
use mool as db;

#[derive(Debug, Clone, Copy, PartialEq, Eq, db::SqlEnum)]
#[sql_enum(rename_all = "snake_case")]
enum PostStatus {
    Draft,
    InReview,
    Published,
}

#[derive(Debug, Clone, db::Model)]
#[table(name = "posts")]
struct Post {
    #[column(primary_key)]
    id: i64,
    author_id: i64,
    title: String,
    #[column(sql_enum)]
    status: PostStatus,
}

#[derive(Debug, Clone, db::Record)]
#[table(name = "posts")]
struct PostPatch {
    title: String,
    status: PostStatus,
}

async fn published<S: db::DBSession>(session: &mut S) -> Result<Vec<Post>, db::DbError> {
    let posts = Post::table();

    db::from(&posts)
        .filter(posts.status.eq(db::val(PostStatus::Published)))
        .order_by(posts.id.desc())
        .all::<Post>()
        .exec(session)
        .await
}
```

## Core Pieces

| Area | What it gives you |
| --- | --- |
| `Model` | Table-backed rows, typed table handles, columns, primary keys, schema metadata. |
| `Record` | Projections, patches, joined records, raw write payloads, and scan metadata. |
| `SqlEnum` | Rust enum to SQL label/code/native type mapping. |
| `Filterable` | Request/search structs converted into typed predicates. |
| Queries | `select`, writes, subqueries, CTEs, variables, functions, aggregates, windows. |
| Relations | Joined records, explicit references, backrefs, many-to-many predicates, prefetch. |
| Sessions | One execution shape for pools, transactions, raw SQL, and mocks. |
| Migrations | [Gaman](https://github.com/vivsh/gaman) schema/migration re-exports plus Mool registries. |

Crates:

- `mool`: runtime crate for applications
- `mool-macros`: derive macros re-exported by `mool`
- `mool-macros-impl`: internal macro implementation

## Models And Records

Use `Model` for table-backed rows:

```rust
#[derive(Debug, Clone, db::Model)]
#[table(name = "posts")]
struct Post {
    #[column(primary_key)]
    id: i64,
    author_id: i64,
    title: String,
    published: bool,
}
```

Use `Record` for projections, patches, joined output, and write-only rows:

```rust
#[derive(Debug, Clone, db::Record)]
#[table(name = "posts")]
struct PostSummary {
    id: i64,
    title: String,
}
```

## Queries

Queries start from a source and finish with a terminal:

```rust
let posts = Post::table();
let author_id = db::var::<i64>().named("author_id");

let rows = db::from(&posts)
    .filter(posts.author_id.eq(&author_id))
    .filter(posts.published.eq(db::val(true)))
    .bind(&author_id, 42_i64)
    .all::<Post>()
    .exec(session)
    .await?;
```

Terminals:

- reads: `all`, `first`, `one`, `slice`, `count`, `exists`, `scalar`
- writes: `insert`, `batch_insert`, `update`, `delete`, `upsert`,
  `batch_upsert`, `returning`
- derived sources: `subquery`, `cte`

## Writes

```rust
let posts = Post::table();
let id = db::var::<i64>().named("id");

db::from(&posts)
    .insert(&PostPatch {
        title: "Hello".to_string(),
        status: PostStatus::Draft,
    })
    .exec(session)
    .await?;

db::from(&posts)
    .filter(posts.id.eq(&id))
    .bind(&id, 1_i64)
    .update(&PostPatch {
        title: "Published".to_string(),
        status: PostStatus::Published,
    })
    .exec(session)
    .await?;
```

## SQL Enums

`SqlEnum` maps fieldless Rust enums to database values.

Storage modes:

| Storage | Backends | Notes |
| --- | --- | --- |
| `text` | Postgres, SQLite3, MySQL | Default. Stores labels and emits check metadata. |
| `int` | Postgres, SQLite3, MySQL | Requires explicit codes and `repr = "i16"`, `"i32"`, or `"i64"`. |
| `native_postgres` | Postgres | Registers native enum schema metadata. |
| `native_mysql` | MySQL | Emits `ENUM(...)` column metadata. MySQL migrations are not managed. |

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, db::SqlEnum)]
#[sql_enum(storage = "int", repr = "i16")]
enum Priority {
    #[sql_enum(code = 1)]
    Low,
    #[sql_enum(code = 2)]
    High,
}
```

Generated helpers:

```rust
PostStatus::SQL_NAME;
PostStatus::SQL_VALUES;
PostStatus::SQL_STORAGE;
PostStatus::Published.as_sql_str();
PostStatus::try_from_sql_str("draft")?;
```

## Filters

`Filterable` turns API/search structs into typed predicates. Empty `Option`,
empty `Vec`, and absent optional lists are skipped.

```rust
#[derive(Debug, Clone, db::Filterable)]
#[filter(model = Post)]
struct PostFilter {
    #[filter(op = "eq")]
    published: Option<bool>,
    #[filter(op = "ilike", column = "title")]
    q: Option<String>,
    #[filter(op = "in", column = "id")]
    ids: Vec<i64>,
}

let rows = db::from(&Post::table())
    .filter_with(&filter)
    .all::<Post>()
    .exec(session)
    .await?;
```

## Relations

Joined records describe references in the output type:

```rust
#[derive(Debug, Clone, db::Model)]
#[table(name = "users")]
struct User {
    #[column(primary_key)]
    id: i64,
    display_name: String,
}

#[derive(Debug, Clone, db::Record)]
struct PostWithAuthor {
    #[column(flatten)]
    post: Post,
    #[column(reference(on(from = "author_id", to = "id")))]
    author: User,
}

let rows = db::from(&Post::table())
    .all::<PostWithAuthor>()
    .exec(session)
    .await?;
```

Backrefs and many-to-many helpers render correlated predicates and aggregates.
Use `prefetch` when child rows should be loaded in a second query.

## Subqueries And CTEs

```rust
#[derive(Debug, Clone, db::Model)]
#[table(name = "comments")]
struct Comment {
    #[column(primary_key)]
    id: i64,
    post_id: i64,
    flagged: bool,
}

#[derive(Debug, Clone, db::Record)]
#[table(name = "comments")]
struct CommentPostId {
    post_id: i64,
}

let comments = Comment::table();
let posts = Post::table();

let visible_post_ids = db::from(&comments)
    .filter(comments.flagged.eq(db::val(false)))
    .all::<CommentPostId>()
    .set(db::out::<CommentPostId>().post_id, &comments.post_id)
    .subquery()?;

let rows = db::from(&posts)
    .filter(posts.id.in_(visible_post_ids.pick(&visible_post_ids.post_id)))
    .all::<Post>()
    .exec(session)
    .await?;
```

Use `cte()` plus `.with(&cte)` when the derived source should be declared in a
`WITH` clause and reused by the parent query.

## Functions

Legend: yes = implemented, no = unsupported.

| Row function | MySQL | SQLite3 | Postgres |
| --- | --- | --- | --- |
| `now`, `coalesce`, `case` | yes | yes | yes |
| ranking windows: `row_number`, `rank`, `dense_rank` | yes | yes | yes |
| distribution windows: `percent_rank`, `cume_dist`, `ntile` | yes | yes | yes |
| value windows: `lag`, `lead`, `first_value`, `last_value`, `nth_value` | yes | yes | yes |
| JSON path helpers | yes | yes | yes |
| `json::postgres::contains` | no | no | yes |
| SQL array helpers | no | no | yes |
| `postgres::unaccent` | no | no | yes |
| custom functions: `funcs::func`, `funcs::custom` | yes | yes | yes |

| Aggregate | MySQL | SQLite3 | Postgres |
| --- | --- | --- | --- |
| `count`, `count_all` | yes | yes | yes |
| `sum`, `avg`, `min`, `max` | yes | yes | yes |

Aggregates work with `group_by`, `having`, scalar terminals, output
assignments, and `over(window())` where the backend supports windows.

## Migrations

With `migrations`, Mool re-exports [Gaman](https://github.com/vivsh/gaman)
schema/migration tools and adds registries for root and crate-owned migration
sources.

```rust
fn schema() -> db::Schema {
    db::schema(db::Dialect::Postgres)
        .model::<Post>()
        .build()
}

let mut registry = db::MigrationRegistry::new();
registry.register_schema(db::root_schema(schema))?;
```

Use `db::schema(...)` instead of raw `SchemaBuilder` when models include native
enum fields.

## Testing

`MockDBSession` records statements and returns planned responses.

```rust
use mool::mock::{DbCallKind, MockDBSession, PlannedCall, PlannedResponse};

let mut session = MockDBSession::new();
session.plan(PlannedCall {
    kind: DbCallKind::FetchAll,
    sql_contains: Some("FROM posts"),
    response: PlannedResponse::OkAnyVec(Box::new(Vec::<Post>::new())),
});

let rows = db::from(&Post::table())
    .all::<Post>()
    .exec(&mut session)
    .await?;
```

## Verification

```sh
cargo test --workspace
cargo check -p mool --no-default-features --features sqlite
cargo check -p mool --no-default-features --features postgres
cargo check -p mool --no-default-features --features mysql
cargo check -p mool --no-default-features --features "sqlite migrations"
cargo check -p mool --no-default-features --features "postgres migrations"
cargo clippy --workspace
```

## Boundary

Mool owns database concerns: pools, sessions, records, models, typed queries,
filters, relations, raw SQL, schema metadata, migrations, enum mappings, and
test mocks.

Framework concerns belong outside Mool: routing, commands, templates, assets,
uploads, task queues, notifications, and UI.
