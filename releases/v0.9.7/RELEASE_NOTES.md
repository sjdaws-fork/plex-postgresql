# Plex PostgreSQL Shim v0.9.7 - macOS Release

## What's New

### Bug Fix: PlayQueues 500 Error
- Fixed `std::exception` when creating play queues
- Root cause: `bind_parameter_index()` returned 0 for pg_stmt with no parameters instead of falling through to SQLite
- All playQueue POST requests now succeed (was 0%, now 100%)

## Installation

1. Stop Plex Media Server
2. Backup original: `cp "/Applications/Plex Media Server.app/Contents/MacOS/Plex Media Server" "/Applications/Plex Media Server.app/Contents/MacOS/Plex Media Server.original"`
3. Copy shim: `cp db_interpose_pg.dylib "/Applications/Plex Media Server.app/Contents/MacOS/"`
4. Create wrapper script or use DYLD_INSERT_LIBRARIES
5. Start Plex Media Server

## Requirements

- macOS (Apple Silicon / ARM64)
- PostgreSQL 15+
- Plex Media Server 1.40+

## Configuration

Set environment variables:
```bash
export PLEX_PG_HOST="/tmp"        # Unix socket path or hostname
export PLEX_PG_PORT="5432"
export PLEX_PG_USER="plex"
export PLEX_PG_PASSWORD=""        # Empty for local socket auth
export PLEX_PG_DATABASE="plex"
export PLEX_PG_SCHEMA="plex"
```

## Changelog

- fix: playQueues 500 error - fallback to SQLite for paramless pg_stmt
