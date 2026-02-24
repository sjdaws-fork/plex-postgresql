# Step Flow Architecture

## Scope

This document describes the runtime flow for `sqlite3_step` interception and the helper module boundaries.

## High-Level Flow

1. `my_sqlite3_step` (retry wrapper)
2. `my_sqlite3_step_impl` (orchestration)
3. Route by statement kind:
   - cached write
   - cached read
   - prepared read
   - prepared write
   - SQLite fallback

Connection-level failures are reported as `SQLITE_ERROR` with `step_pg_conn_error=1`, enabling retry in the wrapper.

## Module Boundaries

- `src/interpose/db_interpose_step.c`
  - orchestration only
  - route decisions and branch sequencing

- `src/interpose/db_interpose_step_read_utils.c`
  - read first execution path
  - streaming/eager/cached row advancement

- `src/interpose/db_interpose_step_write_utils.c`
  - write connection prepare/recovery
  - regular write execute/finalize
  - cached write execute/finalize
  - write policy guards + debug hooks

- `src/interpose/db_interpose_step_cached_read_utils.c`
  - cached-read prepare/execute/finalize helpers

- `src/interpose/db_interpose_conn_utils.c`
  - shared connection cancel+drain utility

- `src/interpose/db_interpose_stmt_lifecycle.c`
  - reset/finalize/clear_bindings interception

## Result/Error Contract

Step helpers use `step_result_t`:

- `STEP_RESULT_ROW`
- `STEP_RESULT_DONE`
- `STEP_RESULT_ERROR`
- `STEP_RESULT_FALLBACK`

For helper APIs with `conn_error_out`:

- `conn_error_out=1` means a connection-level failure occurred and retry is appropriate.
- `conn_error_out=0` means non-connection logic/SQL path.

## Lock Ownership Convention

- Orchestrator enters most helper paths with `pg_stmt->mutex` held.
- Helpers may release `pg_stmt->mutex` on terminal returns.
- `exec_conn->mutex` is acquired/released inside connection/execute helpers.
