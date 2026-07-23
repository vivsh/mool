# Compatibility Policy

## Rust And Databases

Mool's minimum supported Rust version is 1.88. Raising it requires a minor
release and a changelog entry.

Supported database floors are PostgreSQL 15, MySQL 8.0, MariaDB 10.11, and
SQLite 3.35. One backend feature is required per application build. PostgreSQL
and SQLite migrations are supported; MySQL migration maturity remains bounded,
and MariaDB migrations remain experimental until their evidence matrices pass.

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
demonstrate that capability. Release packaging stays disabled while Mool uses an
unreleased path dependency on Gaman HEAD.
