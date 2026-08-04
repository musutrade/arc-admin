#!/usr/bin/env bash
# Canonical local verification entry point: scope -> audit -> tests.
set -euo pipefail

SCRIPT_DIR="$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=/dev/null
source "$SCRIPT_DIR/common.sh" "$SCRIPT_DIR"

bash "$SCRIPT_DIR/changed_paths.sh" "$@"
# scope.env contains only booleans emitted by changed_paths.sh.
# shellcheck source=/dev/null
source "$REPORT_DIR/scope.env"

bash "$SCRIPT_DIR/secret_scan.sh" --all
bash "$SCRIPT_DIR/audit_gate.sh"
RUN_RUST="$RUN_RUST" \
RUN_ANGULAR="$RUN_ANGULAR" \
RUN_AUDITOR="$RUN_AUDITOR" \
RUN_SHELL="$RUN_SHELL" \
  bash "$SCRIPT_DIR/run_tests.sh"
