# Translator Coverage Ledger (Draft)

## Purpose
A living map of SQLite surface area vs. translator support, with direct pointers to the tests that cover each behavior.

## Status Legend
- Supported: intended to work for Plex workloads; tests exist.
- Partial: some patterns supported; gaps exist or behavior is heuristic.
- Skipped/No-op: explicitly ignored by the translator/runtime.
- Gap: known unsupported behavior; tracked in `KNOWN_GAPS.md`.

## Coverage Matrix
| Area | Status | Tests | Notes |
| --- | --- | --- | --- |
| SELECT core (joins, aliases, subqueries) | Supported | `rust/plex-pg-core/src/query.rs` | Core translator rules + fixes. |
| INSERT / UPDATE / DELETE | Supported | `rust/plex-pg-core/src/query.rs`, `rust/plex-pg-core/src/dedup.rs` | UPDATE dedup rule is explicit. |
| INSERT OR REPLACE / IGNORE | Supported | `rust/plex-pg-core/src/upsert.rs`, `rust/plex-pg-core/src/emit.rs` | Mapped to `ON CONFLICT` patterns. |
| Placeholders `?` / `:name` | Supported | `rust/plex-pg-core/src/placeholders.rs` | Deterministic bind ordering. |
| Identifier quoting (backticks) | Supported | `rust/plex-pg-core/src/quotes.rs` | Backticks → double quotes. |
| DISTINCT + ORDER BY | Supported | `rust/plex-pg-core/src/query.rs` (distinct_fix tests) | Adds missing select items. |
| GROUP BY strictness | Supported | `rust/plex-pg-core/src/groupby.rs` | Adds non-aggregate columns. |
| Function rewrites (IFNULL, IIF, STRFTIME, etc.) | Supported | `rust/plex-pg-core/src/functions.rs` | Includes common Plex patterns. |
| JSON functions / operators | Partial | `rust/plex-pg-core/src/functions.rs` | JSONPath fallback used for some patterns. |
| FTS MATCH rewrite | Partial | `rust/plex-pg-core/src/db_interpose_helpers_tests.rs` | Limited to known Plex queries. |
| DDL-lite (CREATE TABLE/INDEX IF NOT EXISTS) | Partial | `rust/plex-pg-core/src/quotes.rs`, `rust/plex-pg-core/src/db_interpose_helpers_tests.rs` | Only patterns used by Plex. |
| PRAGMA / extension hooks | Skipped/No-op | `rust/plex-pg-core/src/pg_config.rs` | Explicit skip list and prefix checks. |
| Transaction control (BEGIN/COMMIT/ROLLBACK) | Partial | `rust/plex-pg-core/src/keywords.rs` | Routed through PG path; semantics differ. |
| Type affinity / decltype mapping | Supported | `rust/plex-pg-core/src/types.rs`, `rust/plex-pg-core/src/pg_statement.rs` | SQLite decltype → PG types. |
| Output validation (PG parser check) | Supported | `rust/plex-pg-core/src/lib.rs` tests | Optional validation gate. |

## Notes
- Known gaps are tracked in `KNOWN_GAPS.md` and the overall intended surface is summarized in `SUPPORTED_SQLITE_SUBSET.md`.
- Test tagging conventions live in `TRANSLATOR_TEST_TAGS.md`; new tests should include tags that map to the matrix above.
- This document is not a guarantee of full SQLite compatibility; it is a practical ledger for the Plex translator.
