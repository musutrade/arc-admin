#!/usr/bin/env bash
# High-confidence local secret scan. Never prints matched secret contents.
set -euo pipefail

SCRIPT_DIR="$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=/dev/null
source "$SCRIPT_DIR/common.sh" "$SCRIPT_DIR"

MODE="${1:---all}"
if [ "$MODE" != "--all" ] && [ "$MODE" != "--staged" ]; then
  echo "Usage: secret_scan.sh [--all|--staged]" >&2
  exit 2
fi

cd "$PROJECT_ROOT"
mkdir -p "$REPORT_DIR"
FILES="$(mktemp "$REPORT_DIR/secret-scan-files.XXXXXX")"
trap 'rm -f "$FILES"' EXIT

if [ "$MODE" = "--staged" ]; then
  git diff --cached --diff-filter=ACMR --name-only -z > "$FILES"
else
  git ls-files --cached --others --exclude-standard -z > "$FILES"
fi

PATTERN='github_pat_[A-Za-z0-9_]{20,}|gh[pousr]_[A-Za-z0-9]{20,}|https?://[^/@[:space:]]+:[^@[:space:]]+@[^[:space:]]+|-----BEGIN ([A-Z0-9 ]+ )?PRIVATE KEY-----'
FAILED=0

while IFS= read -r -d '' file; do
  if [ "$MODE" = "--staged" ]; then
    if git show ":$file" 2>/dev/null | LC_ALL=C grep -aEq "$PATTERN"; then
      echo "Potential secret detected in staged file: $file" >&2
      FAILED=1
    fi
  elif [ -f "$file" ] && LC_ALL=C grep -aEq "$PATTERN" "$file"; then
    echo "Potential secret detected in working-tree file: $file" >&2
    FAILED=1
  fi
done < "$FILES"

if [ "$FAILED" -ne 0 ]; then
  echo "Secret scan failed; remove and revoke the credential before continuing" >&2
  exit 1
fi

echo "Secret scan passed ($MODE)"
