# Changelog

All notable Mool changes are recorded here. The project follows Keep a
Changelog structure and Rust semantic-versioning conventions.

## Unreleased

### Changed

- Rename the multi-row write terminals from `batch_insert`, `batch_upsert`,
  and `batch_update` to `insert_many`, `upsert_many`, and `update_many`.
  `batch_size()` remains the statement-chunking control.
- Require exactly one compile-time backend: PostgreSQL, SQLite, MySQL, or MariaDB.
- Select one renderer and expose backend capabilities through `mool::prelude`.
- Remove public runtime query dialect selection from planning and statements.
- Rename `DBSession` to `DbSession` and add explicit transaction completion.
- Move schema and migration APIs under `mool::schema` and `mool::migrations`.
- Expose SQLx and Gaman only through their namespaced interoperability modules.
- Define `funcs::now()` as statement time on every backend. PostgreSQL callers
  requiring transaction-start time can use
  `funcs::postgres::datetime::transaction_timestamp()`.
- Make CTE and subquery composition infallible; validation now occurs only at
  planning or execution boundaries.
- Qualify root columns with physical source names and omit redundant table
  aliases.
- Split record write metadata into insertable and updateable columns, excluding
  model primary keys from update payloads.
- Replace ambiguous arithmetic helpers with `plus`, `minus`, `times`, and
  `divide_by`; standard Rust arithmetic operators remain supported.
- Replace `Statement::from_str` with the explicit `Statement::raw` constructor.
- Remove callback-based `insert_using`, `update_using`, and `DbPool::transaction`
  APIs. Record-backed `.set(...)` overrides and explicit transactions remain.
- Replace macro token-name matching for temporal and PostgreSQL array columns
  with trait-based Chrono, `time`, UUID, JSON, and SQLx array inference.
- Replace `.order_by(...)` with `.sort(...)` and replace positional pagination
  with `.page(Pagination { page_num, page_size }, session)`.

### Added

- Distinct MariaDB rendering and capability exports.
- Structured database errors with operation context and SQLx sources.
- Savepoint-backed nested transactions.
- Composite-key relation prefetch, batch chunking, row locking, casts, and typed predicates.
- Four-backend compile tests, exact SQL tests, and live CRUD/transaction suites.
- Dialect-neutral batch insert, selective upsert, and primary-key batch update
  with automatic parameter-limit sizing, explicit `BatchPlan` inspection, and
  returning support where available.
- PostgreSQL `UNNEST` inserts and upserts generated from ordinary derived
  records, including PostgreSQL array metadata for `SqlEnum` values.
- Exact conflict ignoring for PostgreSQL and SQLite, plus explicit MySQL-family
  `INSERT IGNORE` support.
- Typed portable datetime extraction, truncation, UTC current values, fixed
  duration arithmetic, backend-specific temporal functions, and optional
  `time` crate integration without replacement temporal value types.
- Alias-safe UUID and JSON model schema inference, including typed
  `sqlx::types::Json<T>` values and SQLx-compatible PostgreSQL arrays.
- Record-backed managed rows for migration-owned configuration and reference
  data, including complete-set insert, update, and reviewed delete planning.
- Application-implemented `Sortable` and `Pageable` traits, plus typed
  `sort_with(...)`, `page_with(...)`, and dialect-aware `random_order()`.

### Compatibility Notes

- An oversized batch operation may now resolve to multiple inspectable
  statements. `.plan()` returns `MultipleStatementsRequired`; use `.plans()`
  to inspect row ranges or `.single_statement()` to require one statement.
- Multi-statement batch execution is not implicitly wrapped in a transaction.

### Fixed

- Multibyte named-placeholder parsing and deterministic empty-list predicates.
- Preservation of SQLx URL transport options while consuming Mool pool options.
- Grouped count/exists semantics, ordered scalar limits, checked pagination
  offsets, CTE source identity, relation-prefetch chunking, and raw duplicate
  bind detection.
