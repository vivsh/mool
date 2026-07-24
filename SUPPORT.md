# Support Policy

Mool supports the latest published `0.2.x` release on Rust 1.88 and later.
Patch releases preserve the published `0.2.x` public API unless a release note
explicitly identifies a security or correctness exception.

## Database Matrix

| Capability | PostgreSQL | SQLite | MySQL | MariaDB |
| --- | --- | --- | --- | --- |
| Typed query planning and execution | Supported | Supported | Supported | Supported |
| Schema metadata | Supported | Supported | Supported | Supported |
| Mool migration workflow | Supported | Supported | Not supported | Not supported |
| Lateral joins | Not supported | Not supported | Not supported | Not supported |

Live compatibility is maintained for PostgreSQL 15 and 18, MySQL 8.0 and 8.4,
MariaDB 10.11 and 11.8, and SQLite through SQLx's bundled driver.

## Compatibility

Mool selects exactly one query backend feature. Backendless builds support
metadata, migration registration, and inert framework compatibility types;
they do not provide query execution, pools connected to a database, or
selected-dialect schema helpers.

Applications own SQLx macros and test infrastructure by declaring SQLx
directly. Mool does not re-export SQLx macros.

## Security

Report vulnerabilities using the private process in [SECURITY.md](SECURITY.md).
