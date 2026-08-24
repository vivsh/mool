# Compatibility Policy

## Rust And Databases

Mool's minimum supported Rust version is 1.88. Raising it requires a minor
release and a changelog entry.

Supported database floors are PostgreSQL 15, MySQL 8.0, MariaDB 10.11, and
SQLite 3.35. At most one backend feature may be selected. Query execution
requires one backend; backendless builds retain metadata and migration
registration. PostgreSQL and SQLite migrations are supported. MySQL and
MariaDB migrations are not supported.

Search support is independently feature-gated and narrower than the general
backend floor:

| Feature | PostgreSQL | MySQL | MariaDB | SQLite |
| --- | --- | --- | --- | --- |
| `text-search` | SQL support; live-version claims require CI fixtures | SQL support; live-version claims require CI fixtures | SQL support; live-version claims require CI fixtures | Unsupported |
| `fuzzy-search` | `pg_trgm`; live-version claims require CI fixtures | Unsupported | Unsupported | Unsupported |
| `vector` | pgvector float vectors; live-version claims require CI fixtures | Unsupported | MariaDB 11.8 target, pending live fixture | Unsupported |

SQL conformance tests are not a substitute for live backend evidence. A
database/version pair must pass its live CRUD, distance/ranking, migration, and
`EXPLAIN` fixture before release documentation calls it supported.

Model schema inference is trait-based for Chrono, `time`, UUID, JSON, and custom
`ColumnType` implementations. PostgreSQL native arrays derive their DDL names
from SQLx type metadata. Explicit `#[column(type = "...")]` annotations remain
authoritative for application-specific storage.

## Semantic Versioning

Mool follows Cargo semantic versioning. Public typed-query behavior includes
generated SQL, bind order, terminal row-count semantics, feature-gated symbol
availability, derive output, and documented error categories. Breaking any of
these contracts requires a major release while the crate is at or above 1.0,
or the corresponding pre-1.0 minor release.

Deprecations should remain for one minor release when a practical bridge exists.
The current backend architecture is an explicitly approved clean break and does
not provide a runtime-dialect compatibility bridge.

## Backend Evidence

Every supported backend must pass formatting, strict Clippy, rustdoc, compile
contracts, deterministic SQL tests, and live CRUD and transaction tests. A
backend-specific API is exported only when its renderer and compile-fail tests
demonstrate that capability. Release packaging requires a published Gaman
version and a registry-only locked dependency graph.
