#!/usr/bin/env bash
# Detect the verification scope and write machine-readable RUN_* flags.
set -euo pipefail

SCRIPT_DIR="$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=/dev/null
source "$SCRIPT_DIR/common.sh" "$SCRIPT_DIR"

usage() {
  cat <<'EOF'
Usage: changed_paths.sh [--working-tree | --staged | --base <ref> | --all]

  --working-tree  Inspect staged, unstaged, and untracked files (default).
  --staged        Inspect only staged files; intended for pre-commit.
  --base <ref>    Inspect committed changes in <ref>...HEAD.
  --all           Force all verification scopes on.
EOF
}

MODE="working-tree"
BASE_REF=""
case "${1:-}" in
  "" | --working-tree) ;;
  --staged) MODE="staged" ;;
  --base)
    MODE="base"
    BASE_REF="${2:-}"
    if [ -z "$BASE_REF" ]; then
      echo "ERROR: --base requires a Git ref" >&2
      usage >&2
      exit 2
    fi
    ;;
  --all) MODE="all" ;;
  -h | --help)
    usage
    exit 0
    ;;
  *)
    echo "ERROR: unsupported argument: $1" >&2
    usage >&2
    exit 2
    ;;
esac

mkdir -p "$REPORT_DIR"
CHANGED_FILE="$REPORT_DIR/changed_files.txt"
SCOPE_FILE="$REPORT_DIR/scope.env"
RAW_FILE="$(mktemp "$REPORT_DIR/changed_paths.XXXXXX")"
trap 'rm -f "$RAW_FILE"' EXIT

cd "$PROJECT_ROOT"

if ! git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  echo "ERROR: project root is not a Git worktree" >&2
  exit 1
fi

case "$MODE" in
  working-tree)
    {
      git diff --name-only
      git diff --cached --name-only
      git ls-files --others --exclude-standard
    } > "$RAW_FILE"
    ;;
  staged)
    git diff --cached --name-only > "$RAW_FILE"
    ;;
  base)
    git rev-parse --verify "$BASE_REF^{commit}" >/dev/null
    git diff --name-only "$BASE_REF...HEAD" > "$RAW_FILE"
    ;;
  all)
    : > "$RAW_FILE"
    ;;
esac

LC_ALL=C sort -u "$RAW_FILE" | sed '/^$/d' > "$CHANGED_FILE"

RUN_RUST=false
RUN_ANGULAR=false
RUN_AUDITOR=false
RUN_SHELL=false

if [ "$MODE" = "all" ]; then
  RUN_RUST=true
  RUN_ANGULAR=true
  RUN_AUDITOR=true
  RUN_SHELL=true
else
  if grep -qE '^backend/' "$CHANGED_FILE"; then
    RUN_RUST=true
  fi
  if grep -qE '^frontend/' "$CHANGED_FILE"; then
    RUN_ANGULAR=true
  fi
  if grep -qE '^rust-toolchain\.toml$' "$CHANGED_FILE"; then
    RUN_RUST=true
    RUN_AUDITOR=true
  fi
  if grep -qE '^\.node-version$' "$CHANGED_FILE"; then
    RUN_ANGULAR=true
  fi
  if grep -qE '^codex-audit-pipeline/tools/auditor/' "$CHANGED_FILE"; then
    RUN_AUDITOR=true
  fi
  if grep -qE '^codex-audit-pipeline/(scripts|hooks)/' "$CHANGED_FILE"; then
    RUN_SHELL=true
    RUN_RUST=true
    RUN_ANGULAR=true
    RUN_AUDITOR=true
  fi
  if grep -qE '^(docs/openapi\.yaml|\.github/workflows/)' "$CHANGED_FILE"; then
    RUN_RUST=true
    RUN_ANGULAR=true
    RUN_AUDITOR=true
  fi
  if grep -qE '^codex-audit-pipeline/\.codex/(audit\.toml|templates/)' "$CHANGED_FILE"; then
    RUN_AUDITOR=true
  fi
fi

{
  printf 'RUN_RUST=%s\n' "$RUN_RUST"
  printf 'RUN_ANGULAR=%s\n' "$RUN_ANGULAR"
  printf 'RUN_AUDITOR=%s\n' "$RUN_AUDITOR"
  printf 'RUN_SHELL=%s\n' "$RUN_SHELL"
} > "$SCOPE_FILE"

if [ -s "$CHANGED_FILE" ]; then
  echo "Changed files: $CHANGED_FILE ($(wc -l < "$CHANGED_FILE") files, mode=$MODE)"
else
  echo "Changed files: none (mode=$MODE)"
fi
cat "$SCOPE_FILE"
