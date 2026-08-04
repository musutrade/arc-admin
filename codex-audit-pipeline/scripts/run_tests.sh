#!/usr/bin/env bash
# Non-mutating verification runner. The final report line is machine-readable.
set -uo pipefail

SCRIPT_DIR="$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=/dev/null
source "$SCRIPT_DIR/common.sh" "$SCRIPT_DIR"

BACKEND_DIR="${BACKEND_DIR:-$PROJECT_ROOT/backend}"
FRONTEND_DIR="${FRONTEND_DIR:-$PROJECT_ROOT/frontend}"
AUDITOR_DIR="$PIPELINE_DIR/tools/auditor"
RUN_RUST="${RUN_RUST:-true}"
RUN_ANGULAR="${RUN_ANGULAR:-true}"
RUN_AUDITOR="${RUN_AUDITOR:-true}"
RUN_SHELL="${RUN_SHELL:-true}"
RUST_TEST_TIMEOUT="${RUST_TEST_TIMEOUT:-120}"
ANGULAR_TEST_TIMEOUT="${ANGULAR_TEST_TIMEOUT:-180}"
ANGULAR_BUILD_TIMEOUT="${ANGULAR_BUILD_TIMEOUT:-180}"
TEST_POSTGRES_IMAGE="${TEST_POSTGRES_IMAGE:-postgres:16-alpine}"
TEST_DB_CONTAINER=""

mkdir -p "$REPORT_DIR/logs"
RAW_LOG_DIR="$REPORT_DIR/logs"
REPORT_FILE="$REPORT_DIR/test_result.md"
: > "$REPORT_FILE"

FAILED=0
SELECTED=0

cleanup() {
  if [ -n "$TEST_DB_CONTAINER" ]; then
    docker rm -f "$TEST_DB_CONTAINER" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT INT TERM

record_failure() {
  FAILED=1
  printf -- '- FAIL: %s\n' "$1" >> "$REPORT_FILE"
}

prepare_test_database() {
  if [ -n "${TEST_DATABASE_URL:-}" ]; then
    return 0
  fi
  if ! command -v docker >/dev/null 2>&1 || ! docker info >/dev/null 2>&1; then
    echo "Docker is required when TEST_DATABASE_URL is not provided" >&2
    return 1
  fi

  TEST_DB_CONTAINER="arc-admin-test-${UID:-0}-$$"
  if ! docker run --rm --detach --pull=never \
    --name "$TEST_DB_CONTAINER" \
    --env POSTGRES_USER=arc_admin_test \
    --env POSTGRES_PASSWORD=arc_admin_test \
    --env POSTGRES_DB=arc_admin_test \
    --publish 127.0.0.1::5432 \
    "$TEST_POSTGRES_IMAGE" >/dev/null; then
    return 1
  fi

  local host_port=""
  local attempt
  for ((attempt = 0; attempt < 30; attempt++)); do
    host_port=$(docker port "$TEST_DB_CONTAINER" 5432/tcp 2>/dev/null | awk -F: 'NR == 1 { print $NF }')
    if [ -n "$host_port" ] \
      && docker exec "$TEST_DB_CONTAINER" pg_isready -U arc_admin_test -d arc_admin_test \
        >/dev/null 2>&1; then
      export TEST_DATABASE_URL="postgres://arc_admin_test:arc_admin_test@127.0.0.1:${host_port}/arc_admin_test"
      return 0
    fi
    sleep 1
  done

  echo "temporary PostgreSQL did not become ready" >&2
  return 1
}

{
  echo "=== Verification report ==="
  echo "Timestamp: $(date -u '+%Y-%m-%dT%H:%M:%SZ')"
  echo "Scope: rust=$RUN_RUST angular=$RUN_ANGULAR auditor=$RUN_AUDITOR shell=$RUN_SHELL"
  echo
} >> "$REPORT_FILE"

echo "[1/5] Static checks"
if [ "$RUN_SHELL" = true ]; then
  SELECTED=$((SELECTED + 1))
  if {
    bash -n "$PIPELINE_DIR"/scripts/*.sh "$PIPELINE_DIR/hooks/pre-commit" \
      && shellcheck -x "$PIPELINE_DIR"/scripts/*.sh "$PIPELINE_DIR/hooks/pre-commit"
  } > "$RAW_LOG_DIR/shellcheck.log" 2>&1; then
    echo "- PASS: Bash syntax and ShellCheck" >> "$REPORT_FILE"
  else
    record_failure "Bash syntax or ShellCheck"
  fi
fi

if [ "$RUN_RUST" = true ]; then
  SELECTED=$((SELECTED + 1))
  if cargo fmt --manifest-path="$BACKEND_DIR/Cargo.toml" -- --check \
    > "$RAW_LOG_DIR/rust_fmt.log" 2>&1 \
    && cargo clippy --manifest-path="$BACKEND_DIR/Cargo.toml" \
      --locked --all-targets --all-features -- -D warnings \
      > "$RAW_LOG_DIR/clippy.log" 2>&1; then
    echo "- PASS: Rust format and Clippy" >> "$REPORT_FILE"
  else
    record_failure "Rust format or Clippy"
  fi
fi

if [ "$RUN_ANGULAR" = true ]; then
  SELECTED=$((SELECTED + 1))
  if (cd "$FRONTEND_DIR" \
    && npm exec --offline eslint -- src --max-warnings=0 \
    && npm run format:check) \
    > "$RAW_LOG_DIR/eslint.log" 2>&1; then
    echo "- PASS: Angular ESLint and format" >> "$REPORT_FILE"
  else
    record_failure "Angular ESLint or format"
  fi
fi

if [ "$RUN_AUDITOR" = true ]; then
  SELECTED=$((SELECTED + 1))
  if cargo fmt --manifest-path="$AUDITOR_DIR/Cargo.toml" -- --check \
    > "$RAW_LOG_DIR/auditor_fmt.log" 2>&1 \
    && cargo clippy --manifest-path="$AUDITOR_DIR/Cargo.toml" \
      --locked --all-targets --all-features -- -D warnings \
      > "$RAW_LOG_DIR/auditor_clippy.log" 2>&1; then
    echo "- PASS: Auditor format and Clippy" >> "$REPORT_FILE"
  else
    record_failure "Auditor format or Clippy"
  fi
fi

echo "[2/5] Compile checks"
if [ "$RUN_RUST" = true ]; then
  if cargo check --manifest-path="$BACKEND_DIR/Cargo.toml" --locked --all-targets \
    > "$RAW_LOG_DIR/cargo_check.log" 2>&1; then
    echo "- PASS: Rust compile" >> "$REPORT_FILE"
  else
    record_failure "Rust compile"
  fi
fi

echo "[3/5] Rust and auditor tests"
if [ "$RUN_RUST" = true ]; then
  RUST_LOG="$RAW_LOG_DIR/rust_test_raw.log"
  if prepare_test_database; then
    timeout "$RUST_TEST_TIMEOUT" cargo test --manifest-path="$BACKEND_DIR/Cargo.toml" \
      --locked -- --nocapture > "$RUST_LOG" 2>&1
    RUST_EXIT=$?
    RUST_COUNT=$(awk '/^running [0-9]+ tests?$/ { total += $2 } END { print total + 0 }' "$RUST_LOG")
    if [ "$RUST_EXIT" -eq 0 ] && [ "$RUST_COUNT" -gt 0 ]; then
      echo "- PASS: Rust tests ($RUST_COUNT)" >> "$REPORT_FILE"
    elif [ "$RUST_EXIT" -eq 0 ]; then
      record_failure "Rust tests executed 0 tests"
    else
      record_failure "Rust tests (exit=$RUST_EXIT, count=$RUST_COUNT)"
    fi
  else
    record_failure "Rust test database setup"
  fi
fi

if [ "$RUN_AUDITOR" = true ]; then
  AUDITOR_LOG="$RAW_LOG_DIR/auditor_test_raw.log"
  timeout "$RUST_TEST_TIMEOUT" cargo test --manifest-path="$AUDITOR_DIR/Cargo.toml" \
    --locked -- --nocapture > "$AUDITOR_LOG" 2>&1
  AUDITOR_EXIT=$?
  AUDITOR_COUNT=$(awk '/^running [0-9]+ tests?$/ { total += $2 } END { print total + 0 }' "$AUDITOR_LOG")
  if [ "$AUDITOR_EXIT" -eq 0 ] && [ "$AUDITOR_COUNT" -gt 0 ]; then
    echo "- PASS: Auditor tests ($AUDITOR_COUNT)" >> "$REPORT_FILE"
  elif [ "$AUDITOR_EXIT" -eq 0 ]; then
    record_failure "Auditor executed 0 tests"
  else
    record_failure "Auditor tests (exit=$AUDITOR_EXIT, count=$AUDITOR_COUNT)"
  fi
fi

echo "[4/5] Angular tests"
if [ "$RUN_ANGULAR" = true ]; then
  NG_LOG="$RAW_LOG_DIR/ng_test_raw.log"
  (cd "$FRONTEND_DIR" && timeout "$ANGULAR_TEST_TIMEOUT" \
    npm test -- --watch=false --runner=vitest) > "$NG_LOG" 2>&1
  NG_EXIT=$?
  ANGULAR_COUNT=$(sed -nE 's/.*Tests[[:space:]]+([0-9]+) passed.*/\1/p' "$NG_LOG" | tail -n 1)
  ANGULAR_COUNT="${ANGULAR_COUNT:-0}"
  if [ "$NG_EXIT" -eq 0 ] && [ "$ANGULAR_COUNT" -gt 0 ]; then
    echo "- PASS: Angular tests ($ANGULAR_COUNT)" >> "$REPORT_FILE"
  elif [ "$NG_EXIT" -eq 0 ]; then
    record_failure "Angular executed 0 tests"
  else
    record_failure "Angular tests (exit=$NG_EXIT, count=$ANGULAR_COUNT)"
  fi
fi

echo "[5/5] Angular production build"
if [ "$RUN_ANGULAR" = true ]; then
  if (cd "$FRONTEND_DIR" && timeout "$ANGULAR_BUILD_TIMEOUT" npm run build) \
    > "$RAW_LOG_DIR/angular_build.log" 2>&1; then
    echo "- PASS: Angular production build" >> "$REPORT_FILE"
  else
    record_failure "Angular production build"
  fi
fi

echo >> "$REPORT_FILE"
if [ "$SELECTED" -eq 0 ]; then
  SUMMARY="TEST_SUMMARY: SKIP"
elif [ "$FAILED" -eq 0 ]; then
  SUMMARY="TEST_SUMMARY: PASS"
else
  SUMMARY="TEST_SUMMARY: FAIL"
fi
echo "$SUMMARY" >> "$REPORT_FILE"

echo "Verification report: $REPORT_FILE"
echo "$SUMMARY"
[ "$FAILED" -eq 0 ]
