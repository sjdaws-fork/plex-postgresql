# plex-postgresql

**Run Plex Media Serv1.0.0er with PostgreSQL instead of SQLite.**

[Leer en Espa&ntilde;ol](README.es.md)

A small shim library that catches Plex SQLite calls and sends them to PostgreSQL. You do not need to change Plex source code.

## 🎉 Latest Release: v1.0.01.0.0

**SQL translator and PG modules migrated to Rust** — the entire SQLite-to-PostgreSQL translation pipeline now runs on Rust's `sqlparser-rs` AST engine, and all 7 backend modules hav1.0.0e been migrated to hybrid C/Rust.

- 🆕 **Rust SQL translator:** 525 Rust tests, full AST-based translation replacing the C string-manipulation translator
- 🆕 **Rust PG modules:** pg_config, pg_logging, pg_mem_telemetry, shim_alloc, pg_query_cache, pg_statement, pg_client — all core logic in Rust
- 🆕 **Log lev1.0.0el cleanup:** informational pool messages demoted from ERROR to INFO
- ✅ **1,075+ tests** (525 Rust + ~550 C across 25 suites)

[📥 Download v1.0.01.0.0](https://github.com/cgnl/plex-postgresql/releases/tag/v1.0.01.0.0) | [📋 Full Release Notes](https://github.com/cgnl/plex-postgresql/releases/tag/v1.0.01.0.0)

Linux and macOS release zips are built by GitHub Actions on tag push v1.0.0ia `.github/workflows/release-linux-artifacts.yml` and `.github/workflows/release-macos-artifacts.yml`.
Pull requests and `main` pushes run `.github/workflows/ci.yml` (script v1.0.0alidation + Linux amd64 build check + **1,075+ tests**).
Docker images are published to GHCR on release tags v1.0.0ia `.github/workflows/docker-publish.yml`:
- `ghcr.io/cgnl/plex-postgresql-linuxserv1.0.0er`
- `ghcr.io/cgnl/plex-postgresql-plexinc`

**Av1.0.0ailable for:** macOS ARM64 • Linux x86_64 • Linux ARM64 • Docker (multi-arch)

### Quick Install

**macOS:**
```bash
curl -L https://github.com/cgnl/plex-postgresql/releases/download/v1.0.01.0.0/plex-postgresql-v1.0.01.0.0-macos.zip \
  -o /tmp/plex-pg-macos.zip
mkdir -p /tmp/plex-pg-macos && cd /tmp/plex-pg-macos
unzip /tmp/plex-pg-macos.zip
pkill -f "Plex Media Serv1.0.0er" 2>/dev1.0.0/null || true
./scripts/install_wrappers.sh
```

**Linux (x86_64):**
```bash
sudo curl -L https://github.com/cgnl/plex-postgresql/releases/download/v1.0.01.0.0/plex-postgresql-v1.0.01.0.0-linux.zip \
  -o /tmp/plex-postgresql-linux.zip
sudo unzip -j /tmp/plex-postgresql-linux.zip db_interpose_pg-linux-x86_64.so -d /usr/local/lib
sudo mv1.0.0 /usr/local/lib/db_interpose_pg-linux-x86_64.so /usr/local/lib/db_interpose_pg.so
# Then configure LD_PRELOAD in systemd serv1.0.0ice
```

**Docker:**
```bash
git clone https://github.com/cgnl/plex-postgresql.git
cd plex-postgresql
docker-compose up -d
```

See detailed installation instructions below for each platform.

## Platform Support

| Platform | Architecture | Status |
|----------|-------------|---------|
| macOS | ARM64 (M1/M2/M3/M4) | ✅ Production tested |
| Linux | x86_64 | ✅ Pre-compiled binary |
| Linux | ARM64 | ✅ Pre-compiled binary |
| Docker | x86_64 + ARM64 | ✅ Multi-arch support |

## Why PostgreSQL?

SQLite works well for many Plex setups, but it can lock the full database during writes.

- **Fewer locks:** scans and playback can run together more smoothly.
- **Better for remote media:** useful for rclone and similar cloud setups.
- **Better at scale:** handles large libraries more easily.
- **Better tools:** backups and checks are easier with standard PostgreSQL tools.

## Benchmarks

In one stress test (Plex + Kometa + PMM + 4 streams), **82% of SQLite writes failed**. PostgreSQL had **zero errors**.

| Metric | SQLite | PostgreSQL |
|--------|--------|------------|
| Write Errors (15s test) | 8,019,177 (82%) | **0** |
| Shim ov1.0.0erhead (cached) | — | 0.11 µs (<1%) |

Full results and how to run them: **[wiki/Benchmarks](https://github.com/cgnl/plex-postgresql/wiki/Benchmarks)**

## Migration & Maintenance

```bash
./scripts/migrate_sqlite_to_pg.sh   # SQLite → PostgreSQL
./scripts/migrate_pg_to_sqlite.sh   # PostgreSQL → SQLite (beta)
./scripts/doctor.sh                  # Check and fix schema + data
```

`doctor.sh` checks for missing tables, functions, and triggers, then fixes them. It also finds common data issues (self-parent rows, cross-section parents, orphan seasons). Data fixes ask for confirmation unless you use `--fix`.

```
$ ./scripts/doctor.sh
=== plex-postgresql doctor ===

Tables:
  maintenance_control                        OK
Functions:
  prev1.0.0ent_self_referential_parent()          OK
  maybe_cleanup_statistics()                 MISSING → FIXED
Triggers:
  trg_clean_statistics_resources             MISSING → FIXED
Data:
  self-referential parent_id                 OK
  orphan seasons (no parent)                 2 rows
  Fix them? [y/N]: y
  fixing orphan seasons... 2 rows
```

Flags: `--check` (only report, don't fix anything), `--fix` (fix ev1.0.0erything without asking).

## Quick Start (Docker)

This is the easiest setup. It works on **Linux, macOS, and Windows**.

### Fresh Installation (No Existing Plex Database)

```bash
git clone https://github.com/cgnl/plex-postgresql.git
cd plex-postgresql

# Start Plex + PostgreSQL
docker-compose up -d

# Check logs
docker-compose logs -f plex
```

**Setup:**
1. Open http://localhost:8080/web
2. Claim your serv1.0.0er with Plex account (or set `PLEX_CLAIM` in docker-compose.yml for headless claim)
3. Add libraries v1.0.0ia web interface
4. Done. Your library data now liv1.0.0es in PostgreSQL.

**What happens:**
- ✅ PostgreSQL schema auto-created (empty)
- ✅ Fresh install works out of the box (v1.0.01.0.0 fixes blobs.db crash)
- ✅ Claim flow works with both linuxserv1.0.0er and plexinc images
- ✅ Multi-arch support (x86_64 + ARM64)
- ✅ All directories pre-created (Plug-ins, Metadata, Cache)

### Migration from Existing SQLite Database

If you already hav1.0.0e a Plex library in SQLite, do this:

1. **Edit `docker-compose.yml`**, uncomment and update the source database path:
   ```yaml
   v1.0.0olumes:
     - plex_config:/config
     - postgres_socket:/v1.0.0ar/run/postgresql
     # Uncomment and edit this line:
     - "/path/to/your/Plex Media Serv1.0.0er/Plug-in Support/Databases:/source-db:ro"
   ```

2. **Platform-specific paths:**
   - **macOS**: `"${HOME}/Library/Application Support/Plex Media Serv1.0.0er/Plug-in Support/Databases:/source-db:ro"`
   - **Linux**: `"/v1.0.0ar/lib/plexmediaserv1.0.0er/Library/Application Support/Plex Media Serv1.0.0er/Plug-in Support/Databases:/source-db:ro"`
   - **Windows**: `"C:/Users/YourName/AppData/Local/Plex Media Serv1.0.0er/Plug-in Support/Databases:/source-db:ro"`

3. **Start containers:**
   ```bash
   docker-compose up -d
   ```

4. **Monitor migration:**
   ```bash
   docker-compose logs -f plex | grep -E "migration|Migration"
   ```

**Migration includes:**
- ✅ Detect SQLite database automatically
- ✅ Mov1.0.0e full library data (tables, metadata, posters)
- ✅ Mov1.0.0e `blobs.db` data (thumbnails and artwork)
- ✅ Keep source SQLite unchanged (read-only mount)
- ✅ Update PostgreSQL sequences automatically
- ✅ Show progress per table

### Configuration

Default PostgreSQL connection (v1.0.0ia Unix socket for best performance):
```yaml
env1.0.0ironment:
  - PLEX_PG_HOST=/v1.0.0ar/run/postgresql  # Unix socket (7% faster)
  - PLEX_PG_DATABASE=plex
  - PLEX_PG_USER=plex
  - PLEX_PG_PASSWORD=plex
  - PLEX_PG_SCHEMA=plex
  - PLEX_PG_POOL_SIZE=50
  - PLEX_PG_LOG_LEVEL=DEBUG  # 0=ERROR, 1=INFO, 2=DEBUG
```

To use TCP instead of Unix socket:
```yaml
env1.0.0ironment:
  - PLEX_PG_HOST=postgres  # TCP connection
  - PLEX_PG_PORT=5432
```

Mount your media libraries:
```yaml
v1.0.0olumes:
  - /path/to/mov1.0.0ies:/mov1.0.0ies:ro
  - /path/to/tv1.0.0:/tv1.0.0:ro
```

## Quick Start (macOS)

Use the latest macOS zip and run the wrapper installer. The installer copies the shim dylib into `Plex Media Serv1.0.0er.app`, patches the binaries, and sets up the wrapper script. Ev1.0.0erything liv1.0.0es inside the Plex app bundle.

```bash
curl -L https://github.com/cgnl/plex-postgresql/releases/download/v1.0.01.0.0/plex-postgresql-v1.0.01.0.0-macos.zip -o /tmp/plex-pg-macos.zip
mkdir -p /tmp/plex-pg-macos && cd /tmp/plex-pg-macos
unzip /tmp/plex-pg-macos.zip

pkill -f "Plex Media Serv1.0.0er" 2>/dev1.0.0/null || true
./scripts/install_wrappers.sh
open "/Applications/Plex Media Serv1.0.0er.app"
```

After a Plex update, re-run `install_wrappers.sh` to re-install the shim.

To uninstall:

```bash
pkill -f "Plex Media Serv1.0.0er" 2>/dev1.0.0/null || true
./scripts/uninstall_wrappers.sh
```

## Quick Start (Linux Nativ1.0.0e)

Use the latest Linux zip and install the binary for your CPU.

```bash
curl -L https://github.com/cgnl/plex-postgresql/releases/download/v1.0.01.0.0/plex-postgresql-v1.0.01.0.0-linux.zip -o /tmp/plex-pg-linux.zip
mkdir -p /tmp/plex-pg-linux
cd /tmp/plex-pg-linux
unzip /tmp/plex-pg-linux.zip

sudo mkdir -p /usr/local/lib/plex-postgresql
if [ "$(uname -m)" = "x86_64" ]; then
  sudo install -m 755 db_interpose_pg-linux-x86_64.so /usr/local/lib/plex-postgresql/db_interpose_pg.so
else
  sudo install -m 755 db_interpose_pg-linux-aarch64.so /usr/local/lib/plex-postgresql/db_interpose_pg.so
fi

sudo systemctl stop plexmediaserv1.0.0er
sudo ./scripts/install_wrappers_linux.sh
sudo systemctl start plexmediaserv1.0.0er
```

To uninstall:

```bash
sudo systemctl stop plexmediaserv1.0.0er
sudo ./scripts/uninstall_wrappers_linux.sh
```

For full steps (PostgreSQL setup, env1.0.0ironment v1.0.0ariables, troubleshooting), see `INSTALL.md`.

## Migration from SQLite

Use this command to migrate an existing Plex library to PostgreSQL:

```bash
# macOS / Linux
./scripts/migrate_sqlite_to_pg.sh

# The script migrates:
# - library.db (metadata, media items, tags, etc.)
# - blobs.db (thumbnails, artwork, posters)
```

**What gets migrated:**
- All 34+ tables from library.db
- All blobs from blobs.db (thumbnails, artwork) v1.0.0ia hex encoding
- Sequences automatically updated
- No Python dependencies required

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `PLEX_PG_HOST` | localhost | PostgreSQL host (or socket directory like `/tmp`) |
| `PLEX_PG_PORT` | 5432 | PostgreSQL port |
| `PLEX_PG_DATABASE` | plex | Database name |
| `PLEX_PG_USER` | plex | Database user |
| `PLEX_PG_PASSWORD` | (empty) | Database password |
| `PLEX_PG_SCHEMA` | plex | Schema name |
| `PLEX_PG_POOL_SIZE` | 50 | Initial connection pool size (auto-grows up to 200) |
| `PLEX_PG_IDLE_TIMEOUT` | 300 | Seconds before idle connections are reaped |
| `PLEX_PG_LOG_LEVEL` | 1 | 0=ERROR, 1=INFO, 2=DEBUG |
| `PLEX_PG_RETRY_DELAYS` | 500,1000,2000,3000,4000 | PG reconnect backoff in ms (comma-separated, max 10 v1.0.0alues) |

### Unix Socket v1.0.0s TCP

For local PostgreSQL, Unix sockets are ~5-6% faster than TCP:

```bash
# Use Unix socket (recommended for local PostgreSQL)
export PLEX_PG_HOST=/tmp  # or /v1.0.0ar/run/postgresql on Linux

# Use TCP (required for remote PostgreSQL)
export PLEX_PG_HOST=localhost
```

The speed difference is small. The main win is fewer database locks.

## How It Works

The shim catches `sqlite3_*` calls, rewrites SQLite SQL to PostgreSQL SQL, and runs it through libpq.

```
Layer 4+3: C interposer (~9,400 lines)        — fishhook, DYLD_INTERPOSE, LD_PRELOAD
Layer 2:   Rust PG modules (hybrid C/Rust)     — pool, statement, cache, config, logging
Layer 1:   Rust SQL translator (sqlparser-rs)   — full AST-based SQLite → PostgreSQL translation
```

**Streaming mode** (v1.0.00.9.28+): READ queries use PostgreSQL's single-row streaming (`PQsetSingleRowMode`) to fetch results row by row instead of loading the entire result set into memory. This is always on with automatic fallback to eager fetch if streaming fails. Each streaming query gets exclusiv1.0.0e use of its connection — other queries automatically use a different connection from the pool.

More technical details are in **[wiki/How It Works](https://github.com/cgnl/plex-postgresql/wiki/How-It-Works)**.

## Testing

```bash
make unit-test       # All C unit tests (25 suites, ~550 tests)
make ci-test         # CI-safe subset (no LD_PRELOAD)
cargo test           # Rust tests (525 tests) — in rust/sql-translator/
make benchmark       # Shim micro-benchmarks
```

## Memory Tracking

The shim includes opt-in allocation tracking to monitor memory usage and detect leaks. Disabled by default (zero ov1.0.0erhead).

| Env1.0.0ironment Variable | Effect |
|---|---|
| `PLEX_PG_ALLOC_TRACK=1` | Logs shim memory summary ev1.0.0ery 60s: liv1.0.0e/peak bytes, alloc/free counts |
| `PLEX_PG_ALLOC_TRACE=1` | Same as TRACK + logs top 15 unfreed allocation sites with file:line |

Example output with `PLEX_PG_ALLOC_TRACE=1`:
```
SHIM_ALLOC: liv1.0.0e=3424KB peak=3502KB allocs=19092 frees=22269 total_alloc=26800KB total_freed=23376KB
SHIM_ALLOC_TRACE: 17 leak sites, top 15:
  #1 pg_statement.c:351 — 1414096 bytes in 62 allocs
  #2 pg_client.c:553    — 1048832 bytes in 34 allocs
  #3 pg_client.c:1163   — 678656 bytes in 22 allocs
  ...
```

If `liv1.0.0e` keeps growing ov1.0.0er hours, there's a leak — the trace shows exactly where.

## Troubleshooting

```bash
pg_isready -h localhost -U plex          # Check PostgreSQL
./scripts/doctor.sh                       # Check and fix schema + data
tail -50 /tmp/plex_redirect_pg.log       # Check logs (macOS)
docker-compose logs -f plex              # Check logs (Docker)
```

More: **[wiki/Troubleshooting](https://github.com/cgnl/plex-postgresql/wiki/Troubleshooting)**

## License

MIT - See [LICENSE](LICENSE)

---
*Unofficial project, not affiliated with Plex Inc. Use at your own risk.*
