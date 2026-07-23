# Mool

[![Crates.io](https://img.shields.io/crates/v/mool.svg)](https://crates.io/crates/mool)
[![Docs.rs](https://docs.rs/mool/badge.svg)](https://docs.rs/mool)
[![License](https://img.shields.io/crates/l/mool.svg)](https://github.com/vivsh/mool)

Mool, pronounced `mool`, means root or source.

Mool is a typed SQL data mapper for Rust and SQLx. It turns Rust models and
schema SQL into deterministic, reviewable migrations while keeping application
queries explicit, typed, and dialect-correct.

## Built for Typed SQL and Automatic Migrations

Mool brings three application-facing guarantees to SQLx projects:

| Strength | What it means in practice |
| --- | --- |
| Typed SQL API | Table handles, columns, values, projections, filters, and relations are generated from Rust types instead of repeated strings. |
| Compile-time dialect safety | Select one backend feature and only its valid query extensions exist. PostgreSQL-only SQL cannot accidentally enter a SQLite or MySQL build. |
| Migration generation | Rust `Model` schema metadata and authored schema SQL define desired state; changed state is diffed into deterministic migration files for review and execution. |

The result is a practical middle ground: direct SQL control, SQLx compatibility,
and a migration workflow that stays close to the application's Rust types.

## Why Mool

Plain SQL remains excellent for one-off statements and carefully tuned queries.
Mool is for the rest: application queries that repeatedly need reliable column
names, bind ordering, row mapping, filters, relations, migrations, and
testable SQL plans.

It is ORM-like where that saves work, but it is not an active-record ORM.
Models do not own a connection or save themselves. You write the operation,
Mool provides the typed metadata, builder, and SQLx execution path.

```rust
use mool as db;
use mool::prelude::*;

#[derive(Debug, Clone, db::Model)]
#[table(name = "posts")]
struct Post {
    #[column(primary_key)]
    id: i64,
    author_id: i64,
    title: String,
    published: bool,
}

let posts = Post::table();
let rows = db::from(&posts)
    .filter(posts.published.eq(db::val(true)))
    .order_by(posts.id.desc())
    .all::<Post>()
    .exec(&mut pool)
    .await?;
```

## Install

Enable exactly one database backend:

```toml
mool = { version = "0.2", features = ["postgres"] }
# or: "sqlite", "mysql", "mariadb"
```

Common optional features:

```toml
mool = { version = "0.2", features = ["postgres", "migrations"] }
mool = { version = "0.2", features = ["sqlite", "mock"] }
```

Automatic migrations are supported for PostgreSQL and SQLite. MySQL and MariaDB
are available as query backends; their migration workflow is still maturing.
Do not use `--all-features`: backend features are exclusive.

`mool::prelude::*` is the normal application import. It includes the common
query and model API plus only the extensions supported by the chosen backend.

## Core Workflow

### Models, records, and writes

Use `Model` for a table-backed row. Use `Record` for projections, patches,
joined output, and write payloads.

```rust
#[derive(Debug, Clone, db::Record)]
#[table(name = "posts")]
struct PostPatch {
    title: String,
    published: bool,
}

let posts = Post::table();
let id = db::var::<i64>().named("id");

db::from(&posts)
    .filter(posts.id.eq(&id))
    .bind(&id, 42_i64)
    .update(&PostPatch {
        title: "A clearer title".to_string(),
        published: true,
    })
    .exec(&mut pool)
    .await?;
```

The same builder handles `insert`, `batch_insert`, `update`, `delete`,
`upsert`, `returning`, `count`, `exists`, `scalar`, and paginated reads.
Plans can be inspected without a database:

```rust
let plan = db::from(&Post::table()).all::<Post>().plan()?;
println!("{}", plan.sql);
```

### Batch inserts, upserts, and updates

Batch writes accept ordinary `Record` or `Model` slices. Mool derives the
largest statement size allowed by the selected backend; `batch_size()` can
lower that limit, while `single_statement()` rejects input that cannot fit.

```rust
db::from(&posts)
    .batch_insert(&new_posts)
    .batch_size(1_000)
    .exec(&mut pool)
    .await?;

db::from(&posts)
    .batch_upsert(&new_posts, (&posts.author_id, &posts.title))
    .update_only(&posts.published)
    .exec(&mut pool)
    .await?;

db::from(&posts)
    .batch_update(&changed_posts, (&posts.title, &posts.published))
    .exec(&mut pool)
    .await?;
```

PostgreSQL and SQLite provide exact `ignore_conflicts()` and
`ignore_conflicts_on(...)` extensions. MySQL and MariaDB provide the broader
`ignore_errors()`, which renders `INSERT IGNORE`. Returning composes with batch
writes on backends that support it; ignored rows are not returned and return
order is unspecified. Affected-row counts follow backend semantics and may not
equal the input length, especially for MySQL-family upserts.

Use `.plans()` to inspect every generated statement and its input row range.
Each planned batch is one session call. Multiple batches are not implicitly
transactional, so wrap execution in an explicit transaction when all rows must
commit or roll back together.

PostgreSQL can transpose derived records into typed arrays and bind one
parameter per writable column:

```rust
use mool::backend::PostgresUnnestExt;

let inserted = db::from(&posts)
    .returning::<Post>()
    .batch_insert(&new_posts)
    .using_unnest()
    .exec(&mut pool)
    .await?;
```

`using_unnest()` is explicit, works with inserts and upserts, and supports
normal models, purpose-built records, nullable values, UUIDs, temporal values,
JSON, and `SqlEnum` fields when their PostgreSQL array representation exists.

### Filters and relations

`Filterable` turns request-shaped structs into typed predicates. Empty optional
values are omitted, so one filter type can serve many search forms.

```rust
#[derive(Debug, Clone, db::Filterable)]
#[filter(model = Post)]
struct PostFilter {
    #[filter(op = "eq")]
    published: Option<bool>,
    #[filter(op = "in", column = "id")]
    ids: Vec<i64>,
}

let rows = db::from(&Post::table())
    .filter_with(&filter)
    .all::<Post>()
    .exec(&mut pool)
    .await?;
```

Models can declare references, and records can flatten joined rows. Mool also
supports back-reference and many-to-many predicates, relation aggregates, and
two-query prefetch when that is more efficient than a join.

### Subqueries, CTEs, and SQL functions

Derived sources remain typed. Build a query, turn it into a `subquery()` or
`cte()`, and use its output handles in the parent query. The expression API
covers comparisons, boolean logic, `IN`, null checks, `CASE`, casts, common
functions, aggregates, windows, and backend-specific capabilities.

Unsupported dialect features are absent at compile time rather than accepted
and rejected later. For example, PostgreSQL-only helpers such as `ILIKE`,
arrays, `DISTINCT ON`, and `RETURNING` are exposed only in PostgreSQL builds.

### Transactions and raw SQL

Transactions use the same explicit lifecycle as SQLx:

```rust
let mut transaction = pool.begin().await?;

db::from(&Post::table())
    .insert(&PostPatch {
        title: "Created in a transaction".to_string(),
        published: false,
    })
    .exec(&mut transaction)
    .await?;

transaction.commit().await?;
```

`DbTransaction::as_sqlx()` gives access to the underlying SQLx transaction
when an operation falls outside Mool's builder. Raw SQL is always available:

```rust
let count = db::query("SELECT COUNT(*) FROM posts WHERE author_id = :author_id")
    .bind("author_id", 42_i64)
    .scalar::<i64>(&mut pool)
    .await?;
```

## SQL Enums

`SqlEnum` maps fieldless Rust enums to database values and works directly with
SQLx binds, model fields, filters, and expressions.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, db::SqlEnum)]
#[sql_enum(rename_all = "snake_case")]
enum PostStatus {
    Draft,
    InReview,
    Published,
}

#[derive(Debug, Clone, db::Model)]
#[table(name = "posts")]
struct PostWithStatus {
    #[column(primary_key)]
    id: i64,
    #[column(sql_enum)]
    status: PostStatus,
}
```

Text labels are portable and the default. Explicit integer codes are available
for compact, stable storage. PostgreSQL native enums are schema-aware; MySQL
native `ENUM` columns are represented in schema metadata.

## Generate Migrations from Your Schema

Models provide schema metadata, including keys, references, checks, indexes,
and enum-backed columns. Build the desired schema directly from Rust models:

```rust
let schema = db::schema()
    .model::<PostWithStatus>()
    .build()?;
```

That Rust-derived schema can be combined with authored schema SQL. The migration
workflow compares desired schema state with the committed migration history,
then generates a deterministic migration for review. Hand-authored SQL remains
available for changes that need it.

With the `migrations` feature, Mool embeds committed migration YAML files and
registers migration history with the application:

```rust
static MIGRATIONS: db::migrations::EmbeddedMigrations =
    db::migrations::embedded_migrations!("migrations");
```

Register the resulting desired schema and embedded history with
`MigrationRegistry` at the application boundary. The generated migration is a
normal reviewed file, not an opaque runtime schema change.

## Testing

Mool supports database-free testing through planned SQL and `MockDbSession`.
The mock records ordered statements and can return planned query responses, so
unit tests can assert application behavior without booting a database.

```rust
let plan = db::from(&Post::table())
    .filter(Post::table().published.eq(db::val(true)))
    .all::<Post>()
    .plan()?;

assert!(plan.sql.contains("WHERE"));
```

The project maintains offline SQL golden tests across supported dialects,
macro compile-contract tests, SQLx compatibility checks, and database-free
examples. Run the local confidence suite with:

```sh
scripts/confidence-check.sh
```

For live backend validation, use `scripts/integration-tests.sh <backend> all`.

## What Mool Provides

| Area | Capability |
| --- | --- |
| Mapping | Models, records, row scanning, typed table and column handles |
| Queries | Selects, batch insert/upsert/update, conflict handling, returning, variables, subqueries, CTEs, functions, aggregates, windows |
| Application queries | Typed filters, relations, backrefs, many-to-many predicates, prefetch, pagination |
| Database access | SQLx pools and transactions, raw SQL, prepared bind metadata, mock sessions |
| Schema | Keys, references, constraints, indexes, custom types, `SqlEnum` metadata |
| Migrations | Desired schema from Rust models and schema SQL, deterministic generated migrations, embedded migration registration |

Mool keeps SQL explicit while removing the repetitive parts of building and
maintaining typed database code.
