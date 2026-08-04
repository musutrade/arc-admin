#!/usr/bin/env bash
# Validate local prerequisites without connecting to the configured database.
set -uo pipefail

SCRIPT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
# shellcheck source=common.sh
source "$SCRIPT_DIR/common.sh" "$SCRIPT_DIR"

failures=0
warnings=0

pass() {
  printf '  [OK] %s\n' "$1"
}

fail() {
  printf '  [FAIL] %s\n' "$1"
  failures=$((failures + 1))
}

warn() {
  printf '  [WARN] %s\n' "$1"
  warnings=$((warnings + 1))
}

check_command() {
  local command_name="$1"
  local version

  if ! command -v "$command_name" >/dev/null 2>&1; then
    fail "missing command: $command_name"
    return
  fi

  version="$("$command_name" --version 2>/dev/null | head -n 1)"
  pass "$command_name${version:+ ($version)}"
}

printf 'arc-admin doctor\n'
printf 'Project root: %s\n\n' "$PROJECT_ROOT"

printf 'Required commands\n'
for command_name in git cargo rustc node npm npx jq timeout; do
  check_command "$command_name"
done

printf '\nProject setup\n'
if [ -d "$PROJECT_ROOT/frontend/node_modules" ]; then
  pass "frontend dependencies are installed"
else
  fail "frontend dependencies are missing; run: cd frontend && npm install"
fi

if [ -n "${DATABASE_URL:-}" ]; then
  pass "DATABASE_URL is exported"
elif [ -f "$PROJECT_ROOT/backend/.env" ] \
  && grep -Eq '^[[:space:]]*DATABASE_URL[[:space:]]*=' "$PROJECT_ROOT/backend/.env"; then
  pass "backend/.env defines DATABASE_URL (connectivity not tested)"
else
  fail "DATABASE_URL is not configured; create backend/.env from backend/.env.example"
fi

if find "$PROJECT_ROOT/backend/migrations" -maxdepth 1 -type f -name '*.sql' -print -quit \
  | grep -q .; then
  pass "backend migrations are present"
else
  fail "no SQL migrations found in backend/migrations"
fi

expected_hooks_path="codex-audit-pipeline/hooks"
actual_hooks_path="$(git -C "$PROJECT_ROOT" config --get core.hooksPath 2>/dev/null || true)"
if [ "$actual_hooks_path" = "$expected_hooks_path" ]; then
  pass "Git hooks path is configured"
else
  warn "Git hooks path is '$actual_hooks_path'; expected '$expected_hooks_path'"
fi

if [ -x "$AUDITOR_BIN" ]; then
  pass "auditor binary is built"
else
  warn "auditor binary is not built; audit_gate.sh will build it on first run"
fi

printf '\nSummary: %d failure(s), %d warning(s)\n' "$failures" "$warnings"
if [ "$failures" -gt 0 ]; then
  exit 1
fi
