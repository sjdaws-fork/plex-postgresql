# plex-postgresql

**Draai Plex Media Server met PostgreSQL in plaats van SQLite.**

[Read in English](README.md) | [Leer en Espa&ntilde;ol](README.es.md)

Een kleine shim-bibliotheek die de SQLite-aanroepen van Plex onderschept en doorstuurt naar PostgreSQL. Je hoeft Plex niet aan te passen.

| Platform | Status |
|----------|--------|
| macOS | ✅ Getest in productie |
| Linux (Docker) | ✅ Werkt (init en uitvoering getest) |
| Linux (Natief) | ✅ Voorgecompileerde binaries |

## Laatste versie: v1.2.0

**Rust-safe shim** — 92% van de onveilige raw pointer dereferences ge-elimineerd. De bedrijfslogica is nu Rust-safe; unsafe is beperkt tot de FFI-grens.

- 🆕 **92% unsafe reductie** — raw pointer dereferences 806→59, interne functies gebruiken veilige `&mut` referenties
- 🆕 **Geheugenlek opgelost (1.8GB→59MB)** — PGresult-lek, transactieroutering, cached_result opruiming
- 🔧 **3 deadlocks opgelost** — recursieve connection mutex, ABBA-preventie, convoyfix
- 🔧 **11 data races gefixt** — atomaire tellers, seqlocks, OnceLock, eager hook resolutie
- 🔧 **PgStmt met Vec** — on-demand allocatie, 540 bytes voor queries zonder parameters (was 88KB)
- ✅ **Clippy schoon** — nul waarschuwingen met `-D warnings`

Download: https://github.com/cgnl/plex-postgresql/releases/tag/v1.2.0

## Waarom PostgreSQL?

SQLite werkt goed in veel gevallen, maar heeft een belangrijke beperking: **database-brede vergrendeling**.

- **Minder vergrendelingen** — met PostgreSQL kunnen scans en afspelen beter naast elkaar draaien.
- **Beter voor remote opslag** — handig met rclone en cloudopslagdiensten.
- **Beter bij grote bibliotheken** — verwerkt grote catalogi stabieler.
- **Bekende tools** — `pg_dump` en PostgreSQL-clients voor backup en controle.

## Snel starten (Docker)

De eenvoudigste manier om Plex met PostgreSQL te draaien:

```bash
git clone https://github.com/cgnl/plex-postgresql.git
cd plex-postgresql

# Start Plex + PostgreSQL
docker compose up -d

# Bekijk logs
docker compose logs -f plex
```

Plex is beschikbaar op http://localhost:8080

## Snel starten (macOS)

```bash
curl -L https://github.com/cgnl/plex-postgresql/releases/download/v1.2.0/plex-postgresql-v1.2.0-macos.zip \
  -o /tmp/plex-pg-macos.zip
mkdir -p /tmp/plex-pg-macos && cd /tmp/plex-pg-macos
unzip /tmp/plex-pg-macos.zip
pkill -f "Plex Media Server" 2>/dev/null || true
./scripts/install_wrappers.sh
```

Na een Plex-update: voer `install_wrappers.sh` opnieuw uit.

## Snel starten (Linux)

```bash
curl -L https://github.com/cgnl/plex-postgresql/releases/download/v1.2.0/plex-postgresql-v1.2.0-linux.zip \
  -o /tmp/plex-pg-linux.zip
mkdir -p /tmp/plex-pg-linux && cd /tmp/plex-pg-linux
unzip /tmp/plex-pg-linux.zip

sudo mkdir -p /usr/local/lib/plex-postgresql
if [ "$(uname -m)" = "x86_64" ]; then
  sudo install -m 755 db_interpose_pg-linux-x86_64.so /usr/local/lib/plex-postgresql/db_interpose_pg.so
else
  sudo install -m 755 db_interpose_pg-linux-aarch64.so /usr/local/lib/plex-postgresql/db_interpose_pg.so
fi

sudo systemctl stop plexmediaserver
sudo ./scripts/install_wrappers_linux.sh
sudo systemctl start plexmediaserver
```

## Migratie van SQLite

```bash
./scripts/migrate_sqlite_to_pg.sh   # SQLite → PostgreSQL
./scripts/migrate_pg_to_sqlite.sh   # PostgreSQL → SQLite (beta)
./scripts/doctor.sh                  # Controleer en repareer schema + data
```

## Configuratie

| Variabele | Standaard | Beschrijving |
|-----------|-----------|-------------|
| `PLEX_PG_HOST` | localhost | PostgreSQL host (of socket-map zoals `/tmp`) |
| `PLEX_PG_PORT` | 5432 | PostgreSQL poort |
| `PLEX_PG_DATABASE` | plex | Databasenaam |
| `PLEX_PG_USER` | plex | Databasegebruiker |
| `PLEX_PG_PASSWORD` | (leeg) | Databasewachtwoord |
| `PLEX_PG_SCHEMA` | plex | Schemanaam |
| `PLEX_PG_POOL_SIZE` | 50 | Initieel connection pool formaat |
| `PLEX_PG_LOG_LEVEL` | 1 | 0=ERROR, 1=INFO, 2=DEBUG |

### Unix Socket vs TCP

Voor lokale PostgreSQL zijn Unix sockets ~5-6% sneller dan TCP:

```bash
# Unix socket (aanbevolen voor lokaal)
export PLEX_PG_HOST=/tmp

# TCP (nodig voor remote PostgreSQL)
export PLEX_PG_HOST=localhost
```

## Hoe het werkt

De shim vangt `sqlite3_*`-aanroepen op, herschrijft SQLite SQL naar PostgreSQL SQL, en voert het uit via libpq.

```
Laag 4+3: Rust interposer          — fishhook, DYLD_INTERPOSE, LD_PRELOAD
Laag 2:   Rust PG-modules          — pool, statement, cache, config, logging
Laag 1:   Rust SQL-vertaler         — volledig AST-gebaseerde SQLite → PostgreSQL vertaling
```

**Streamingmodus** (v0.9.28+): READ-queries gebruiken PostgreSQL's single-row streaming (`PQsetSingleRowMode`) om resultaten rij voor rij op te halen in plaats van het hele resultaat in het geheugen te laden.

Meer technische details in de **[wiki](https://github.com/cgnl/plex-postgresql/wiki)**.

## Testen

```bash
cargo test --manifest-path rust/plex-pg-core/Cargo.toml   # Alle Rust tests
```

## Probleemoplossing

```bash
pg_isready -h localhost -U plex          # Controleer PostgreSQL
./scripts/doctor.sh                       # Controleer en repareer schema + data
tail -50 /tmp/plex_redirect_pg.log       # Bekijk logs (macOS)
docker compose logs -f plex              # Bekijk logs (Docker)
```

Meer: **[wiki/Troubleshooting](https://github.com/cgnl/plex-postgresql/wiki/Troubleshooting)**

## Licentie

MIT - Zie [LICENSE](LICENSE)

---
*Onofficieel project, niet gelieerd aan Plex Inc. Gebruik op eigen risico.*
