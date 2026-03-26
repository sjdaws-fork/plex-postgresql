#!/bin/bash
# audit_metadata_parents.sh - inspect metadata_items parent-chain integrity
#
# Focused, read-only audit for the parent/child invariants that matter for
# Plex recursion and metadata integrity.
#
# Usage:
#   ./scripts/audit_metadata_parents.sh
#   ./scripts/audit_metadata_parents.sh --fail-on-issues

set -euo pipefail

FAIL_ON_ISSUES=0

case "${1:-}" in
    "")
        ;;
    --fail-on-issues)
        FAIL_ON_ISSUES=1
        ;;
    -h|--help)
        cat <<'EOF'
Usage: audit_metadata_parents.sh [--fail-on-issues]

Read-only audit for metadata_items parent integrity.

Environment:
  PSQL_BIN           Optional explicit psql path
  PLEX_PG_HOST       Default: localhost
  PLEX_PG_PORT       Default: 5432
  PLEX_PG_DATABASE   Default: plex
  PLEX_PG_USER       Default: plex
  PLEX_PG_PASSWORD   Default: plex
  PLEX_PG_SCHEMA     Default: plex
EOF
        exit 0
        ;;
    *)
        echo "Usage: $0 [--fail-on-issues]" >&2
        exit 2
        ;;
esac

PSQL_BIN="${PSQL_BIN:-/opt/homebrew/opt/postgresql@15/bin/psql}"
if ! command -v "$PSQL_BIN" >/dev/null 2>&1; then
    PSQL_BIN="psql"
fi
if ! command -v "$PSQL_BIN" >/dev/null 2>&1; then
    echo "ERROR: psql not found" >&2
    exit 1
fi

PG_HOST="${PLEX_PG_HOST:-localhost}"
PG_PORT="${PLEX_PG_PORT:-5432}"
PG_DATABASE="${PLEX_PG_DATABASE:-plex}"
PG_USER="${PLEX_PG_USER:-plex}"
PG_SCHEMA="${PLEX_PG_SCHEMA:-plex}"

export PGHOST="$PG_HOST"
export PGPORT="$PG_PORT"
export PGDATABASE="$PG_DATABASE"
export PGUSER="$PG_USER"
export PGPASSWORD="${PLEX_PG_PASSWORD:-plex}"

run_count() {
    local sql="$1"
    local out
    out=$("$PSQL_BIN" -t -A -c "$sql" 2>/dev/null | tr -d '[:space:]')
    if [[ -z "$out" ]]; then
        out="0"
    fi
    if [[ ! "$out" =~ ^[0-9]+$ ]]; then
        echo "ERROR: unexpected psql output for audit query: $out" >&2
        exit 1
    fi
    printf '%s\n' "$out"
}

report_count() {
    local label="$1"
    local value="$2"
    if (( value > 0 )); then
        printf "  %-42s %s rows\n" "$label" "$value"
        issue_categories=$((issue_categories + 1))
        aggregate_rows=$((aggregate_rows + value))
    else
        printf "  %-42s OK\n" "$label"
    fi
}

echo "=== metadata_items parent audit ==="
echo "PostgreSQL: $PG_USER@$PG_HOST:$PG_PORT/$PG_DATABASE (schema: $PG_SCHEMA)"
echo ""

if ! "$PSQL_BIN" -c "SELECT 1" >/dev/null 2>&1; then
    echo "ERROR: cannot connect to PostgreSQL" >&2
    exit 1
fi

self_ref_sql="SELECT COUNT(*) FROM $PG_SCHEMA.metadata_items WHERE parent_id = id;"
dangling_parent_sql=$(cat <<SQL
SELECT COUNT(*) FROM $PG_SCHEMA.metadata_items m
LEFT JOIN $PG_SCHEMA.metadata_items p ON p.id = m.parent_id
WHERE m.parent_id IS NOT NULL
  AND p.id IS NULL;
SQL
)
cross_section_sql=$(cat <<SQL
SELECT COUNT(*) FROM $PG_SCHEMA.metadata_items m
JOIN $PG_SCHEMA.metadata_items p ON p.id = m.parent_id
WHERE m.parent_id IS NOT NULL
  AND m.library_section_id IS NOT NULL
  AND p.library_section_id IS NOT NULL
  AND m.library_section_id != p.library_section_id;
SQL
)
show_bad_parent_sql=$(cat <<SQL
SELECT COUNT(*) FROM $PG_SCHEMA.metadata_items m
JOIN $PG_SCHEMA.metadata_items p ON p.id = m.parent_id
WHERE m.metadata_type = 2
  AND p.metadata_type != 18;
SQL
)
season_orphan_sql="SELECT COUNT(*) FROM $PG_SCHEMA.metadata_items WHERE metadata_type = 3 AND parent_id IS NULL;"
season_bad_parent_sql=$(cat <<SQL
SELECT COUNT(*) FROM $PG_SCHEMA.metadata_items m
JOIN $PG_SCHEMA.metadata_items p ON p.id = m.parent_id
WHERE m.metadata_type = 3
  AND p.metadata_type NOT IN (2, 18);
SQL
)
episode_bad_parent_sql=$(cat <<SQL
SELECT COUNT(*) FROM $PG_SCHEMA.metadata_items m
JOIN $PG_SCHEMA.metadata_items p ON p.id = m.parent_id
WHERE m.metadata_type = 4
  AND p.metadata_type NOT IN (3, 4, 18);
SQL
)
direct_cycles_sql=$(cat <<SQL
SELECT COUNT(*) FROM $PG_SCHEMA.metadata_items m
JOIN $PG_SCHEMA.metadata_items p ON p.id = m.parent_id
WHERE p.parent_id = m.id
  AND m.id < p.id;
SQL
)
cyclic_chains_sql=$(cat <<SQL
WITH RECURSIVE walk AS (
    SELECT
        m.id AS start_id,
        m.parent_id AS next_parent_id,
        ARRAY[m.id] AS path,
        0 AS depth,
        false AS cycle
    FROM $PG_SCHEMA.metadata_items m
    WHERE m.parent_id IS NOT NULL

    UNION ALL

    SELECT
        walk.start_id,
        p.parent_id AS next_parent_id,
        walk.path || p.id,
        walk.depth + 1,
        p.id = ANY(walk.path) AS cycle
    FROM walk
    JOIN $PG_SCHEMA.metadata_items p ON p.id = walk.next_parent_id
    WHERE walk.next_parent_id IS NOT NULL
      AND NOT walk.cycle
      AND walk.depth < 63
)
SELECT COUNT(DISTINCT start_id) FROM walk WHERE cycle;
SQL
)
junk_items_sql="SELECT COUNT(*) FROM $PG_SCHEMA.metadata_items WHERE metadata_type IS NULL AND library_section_id IS NULL;"

issue_categories=0
aggregate_rows=0

report_count "self-referential parent_id" "$(run_count "$self_ref_sql")"
report_count "dangling parent_id" "$(run_count "$dangling_parent_sql")"
report_count "cross-section parent_id" "$(run_count "$cross_section_sql")"
report_count "shows with non-collection parent" "$(run_count "$show_bad_parent_sql")"
report_count "orphan seasons" "$(run_count "$season_orphan_sql")"
report_count "seasons with invalid parent type" "$(run_count "$season_bad_parent_sql")"
report_count "episodes with invalid parent type" "$(run_count "$episode_bad_parent_sql")"
report_count "direct parent cycles" "$(run_count "$direct_cycles_sql")"
report_count "cyclic parent chains" "$(run_count "$cyclic_chains_sql")"
report_count "junk metadata_items" "$(run_count "$junk_items_sql")"

echo ""
echo "Summary:"
printf "  %-42s %s\n" "issue categories" "$issue_categories"
printf "  %-42s %s\n" "aggregate suspicious rows (overlap possible)" "$aggregate_rows"

if (( FAIL_ON_ISSUES == 1 && issue_categories > 0 )); then
    exit 1
fi
