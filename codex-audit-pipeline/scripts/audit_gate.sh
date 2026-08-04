#!/bin/bash
# 确定性审计门禁：auditor 全量扫描，存在任何违规即退出非 0
# 判定依据是每次运行生成的完整 review_context.json。
# 用法（仓库根）：bash codex-audit-pipeline/scripts/audit_gate.sh
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=/dev/null
source "$SCRIPT_DIR/common.sh" "$SCRIPT_DIR"

mkdir -p "$REPORT_DIR"

# Always invoke Cargo so source changes cannot leave a stale release binary.
(cd "$PIPELINE_DIR/tools/auditor" && cargo build --release --locked --quiet)

(
  cd "$PROJECT_ROOT"
  AUDITOR_CONFIG="$AUDITOR_CONFIG" AUDITOR_REPORT_DIR="$REPORT_DIR" \
    "$AUDITOR_BIN" audit >/dev/null
)

REPORT_JSON="$REPORT_DIR/review_context.json"
jq -e '.summary | .total_violations >= 0' "$REPORT_JSON" >/dev/null
TOTAL=$(jq -r '.summary.total_violations' "$REPORT_JSON")
BLOCKER=$(jq -r '.summary.blocker_count' "$REPORT_JSON")
ERRORS=$(jq -r '.summary.error_count' "$REPORT_JSON")

echo "审计结果: total=$TOTAL blocker=$BLOCKER error=$ERRORS"

if [ "$TOTAL" -gt 0 ]; then
  echo "以下为违规明细（前 10 条，完整见 $REPORT_DIR/review_context.json）："
  jq -r '
    (.hard_violations[] | select(.count > 0) | "⛔ [" + .severity + "] " + .rule + " x" + (.count|tostring),
     (.occurrences[0:5][] | "     " + .file + ":" + (.line|tostring) + "  " + .content)),
    (.arch_violations[] | select(.count > 0) | "🏗 [" + .layer + "] " + .rule + " x" + (.count|tostring),
     (.occurrences[0:5][] | "     " + .file + ":" + (.line|tostring) + "  " + .content))
  ' "$REPORT_JSON" | head -40
  echo "❌ 审计门禁未通过：存在违规，请按行号修复后重跑"
  exit 1
fi

echo "✅ 审计门禁通过，无违规"
