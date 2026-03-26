use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn make_temp_dir(name: &str) -> PathBuf {
    let unique = format!(
        "{}-{}-{}",
        std::process::id(),
        name,
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let dir = std::env::temp_dir().join(unique);
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn write_executable(path: &Path, body: &str) {
    fs::write(path, body).expect("write executable");
    let mut perms = fs::metadata(path).expect("metadata").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).expect("chmod");
}

fn fake_psql_script() -> String {
    r#"#!/bin/sh
set -eu

log_file="${FAKE_PSQL_LOG:?}"
query=""
while [ "$#" -gt 0 ]; do
  if [ "$1" = "-c" ]; then
    query="$2"
    break
  fi
  shift
done

if [ -z "$query" ]; then
  echo "missing -c query" >&2
  exit 2
fi

printf '%s\n--\n' "$query" >> "$log_file"

case "$query" in
  "SELECT 1"*)
    printf '1\n'
    ;;
  *"WHERE parent_id = id;"*)
    printf '%s\n' "${FAKE_SELF_REF:-0}"
    ;;
  *"LEFT JOIN plex.metadata_items p ON p.id = m.parent_id"*)
    printf '%s\n' "${FAKE_DANGLING_PARENT:-0}"
    ;;
  *"m.library_section_id != p.library_section_id;"*)
    printf '%s\n' "${FAKE_CROSS_SECTION:-0}"
    ;;
  *"WHERE m.metadata_type = 2"* )
    printf '%s\n' "${FAKE_SHOW_BAD_PARENT:-0}"
    ;;
  *"WHERE metadata_type = 3 AND parent_id IS NULL;"*)
    printf '%s\n' "${FAKE_ORPHAN_SEASONS:-0}"
    ;;
  *"WHERE m.metadata_type = 3"* )
    printf '%s\n' "${FAKE_SEASON_BAD_PARENT:-0}"
    ;;
  *"WHERE m.metadata_type = 4"* )
    printf '%s\n' "${FAKE_EPISODE_BAD_PARENT:-0}"
    ;;
  *"WHERE p.parent_id = m.id"* )
    printf '%s\n' "${FAKE_DIRECT_CYCLES:-0}"
    ;;
  *"WITH RECURSIVE walk AS"* )
    printf '%s\n' "${FAKE_CYCLIC_CHAINS:-0}"
    ;;
  *"metadata_type IS NULL AND library_section_id IS NULL;"*)
    printf '%s\n' "${FAKE_JUNK_ITEMS:-0}"
    ;;
  *)
    printf '0\n'
    ;;
esac
"#
    .to_string()
}

fn run_audit(temp_dir: &Path, extra_env: &[(&str, &str)], args: &[&str]) -> Output {
    let fake_psql = temp_dir.join("psql");
    let log_file = temp_dir.join("queries.log");
    write_executable(&fake_psql, &fake_psql_script());

    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("scripts/audit_metadata_parents.sh");

    let mut cmd = Command::new("bash");
    cmd.arg(script)
        .args(args)
        .env("PSQL_BIN", &fake_psql)
        .env("FAKE_PSQL_LOG", &log_file)
        .env("PLEX_PG_SCHEMA", "plex");

    for (k, v) in extra_env {
        cmd.env(k, v);
    }

    cmd.output().expect("run audit script")
}

#[test]
fn audit_script_reports_issue_categories_and_uses_recursive_cycle_query() {
    let temp_dir = make_temp_dir("audit-script-fail");
    let output = run_audit(
        &temp_dir,
        &[
            ("FAKE_SELF_REF", "2"),
            ("FAKE_DIRECT_CYCLES", "1"),
            ("FAKE_CYCLIC_CHAINS", "4"),
            ("FAKE_ORPHAN_SEASONS", "3"),
        ],
        &["--fail-on-issues"],
    );

    assert_eq!(output.status.code(), Some(1), "{:?}", output);

    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    assert!(stdout.contains("self-referential parent_id"));
    assert!(stdout.contains("direct parent cycles"));
    assert!(stdout.contains("cyclic parent chains"));
    assert!(stdout.contains("issue categories"));

    let query_log = fs::read_to_string(temp_dir.join("queries.log")).expect("query log");
    assert!(query_log.contains("WITH RECURSIVE walk AS"));
    assert!(query_log.contains("LEFT JOIN plex.metadata_items p ON p.id = m.parent_id"));
}

#[test]
fn audit_script_succeeds_cleanly_when_no_issues_are_found() {
    let temp_dir = make_temp_dir("audit-script-pass");
    let output = run_audit(&temp_dir, &[], &["--fail-on-issues"]);

    assert!(
        output.status.success(),
        "status={:?} stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    assert!(stdout.contains("self-referential parent_id"));
    assert!(stdout.contains("OK"));
    assert!(stdout.contains("issue categories                           0"));
}
