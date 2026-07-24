#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/integration-tests.sh [sqlite|postgres|mysql|mariadb|all] [smoke|all]

Environment:
  POSTGRES_DATABASE_URL  Base Postgres URL for sqlx::test.
                         Example: postgres://postgres:postgres@localhost/mool_tests
  MYSQL_DATABASE_URL     Base MySQL URL for sqlx::test.
                         Example: mysql://root:password@localhost/mool_tests
  MARIADB_DATABASE_URL   Base MariaDB URL for sqlx::test.
                         Example: mysql://root:password@localhost/mool_tests
  DATABASE_URL           Fallback URL when running a single server engine.

Notes:
  SQLite uses a temporary file database created by this script.
  Postgres, MySQL, and MariaDB use sqlx::test, which creates a temporary test
  database from the provided base URL and drops it after the test run.
  Suite "smoke" runs CRUD interoperability only. Suite "all" also verifies
  commit, rollback, savepoint, and rollback-on-drop transaction behavior.
USAGE
}

repo_root() {
  local script_dir
  script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  cd "${script_dir}/.." && pwd
}

load_env_file() {
  if [[ -f ".env" ]]; then
    set -a
    # shellcheck disable=SC1091
    source ".env"
    set +a
  fi
}

server_database_url() {
  local engine="$1"
  local specific_var
  local env_name

  case "${engine}" in
    postgres)
      env_name="POSTGRES_DATABASE_URL"
      specific_var="${POSTGRES_DATABASE_URL:-}"
      ;;
    mysql)
      env_name="MYSQL_DATABASE_URL"
      specific_var="${MYSQL_DATABASE_URL:-}"
      ;;
    mariadb)
      env_name="MARIADB_DATABASE_URL"
      specific_var="${MARIADB_DATABASE_URL:-}"
      ;;
    *) echo "unsupported server engine: ${engine}" >&2; return 2 ;;
  esac

  if [[ -n "${specific_var}" ]]; then
    printf '%s\n' "${specific_var}"
    return 0
  fi

  if [[ "${selected_engine}" != "all" && -n "${DATABASE_URL:-}" ]]; then
    printf '%s\n' "${DATABASE_URL}"
    return 0
  fi

  echo "missing ${engine} database URL" >&2
  echo "set ${env_name}, or DATABASE_URL when running only ${engine}" >&2
  return 1
}

run_sqlite() {
  local tmp_dir
  tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/mool-sqlite.XXXXXX")"
  trap "rm -rf '${tmp_dir}'" RETURN

  echo "==> sqlite"
  DATABASE_URL="sqlite://${tmp_dir}/mool.sqlite" \
    run_cargo_suite sqlite
}

run_server_engine() {
  local engine="$1"
  local url
  url="$(server_database_url "${engine}")"

  echo "==> ${engine}"
  DATABASE_URL="${url}" \
    run_cargo_suite "${engine}"
}

run_cargo_suite() {
  local engine="$1"

  if [[ "${selected_suite}" == "smoke" ]]; then
    cargo test --locked -p mool --no-default-features --features "${engine}" \
      --test sqlx_smoke -- --ignored
    return
  fi

  cargo test --locked -p mool --no-default-features --features "${engine} time" \
    --test sqlx_smoke --test sqlx_transactions --test batch_writes_sqlx \
    --test datetime_sqlx -- --ignored
}

main() {
  selected_engine="${1:-all}"
  selected_suite="${2:-smoke}"

  case "${selected_engine}" in
    -h|--help)
      usage
      return 0
      ;;
    sqlite|postgres|mysql|mariadb|all)
      ;;
    *)
      echo "unknown engine: ${selected_engine}" >&2
      usage >&2
      return 2
      ;;
  esac

  case "${selected_suite}" in
    smoke|all)
      ;;
    *)
      echo "unknown suite: ${selected_suite}" >&2
      usage >&2
      return 2
      ;;
  esac

  cd "$(repo_root)"
  load_env_file

  case "${selected_engine}" in
    sqlite)
      run_sqlite
      ;;
    postgres)
      run_server_engine postgres
      ;;
    mysql)
      run_server_engine mysql
      ;;
    mariadb)
      run_server_engine mariadb
      ;;
    all)
      run_sqlite
      run_server_engine postgres
      run_server_engine mysql
      run_server_engine mariadb
      ;;
  esac
}

main "$@"
