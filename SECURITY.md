# Security Policy

## Supported Versions

Security fixes are applied to the latest published minor release. Older minor
releases may receive a fix when the issue is severe and a backport is practical.
Unreleased branches are not production support targets.

## Reporting

Report suspected vulnerabilities privately through GitHub Security Advisories
for `vivsh/mool`. Do not open a public issue containing exploit details, database
credentials, connection URLs, bound values, or production SQL logs.

Include the Mool version, selected backend feature, Rust version, minimal
reproduction, and expected impact. Receipt should be acknowledged within seven
days. A coordinated disclosure date will be agreed after impact is confirmed.

Mool tracing never logs bound values by default. Applications remain responsible
for protecting database URLs and for reviewing any custom tracing layers or raw
SQL logging they add around SQLx.
