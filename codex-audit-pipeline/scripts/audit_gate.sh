#!/bin/bash
# 确定性审计门禁：auditor 全量扫描，存在任何违规即退出非 0
# 判定依据是完整 JSON（review_context.json），不是截断版 markdown
# 用法（仓库根）：bash codex-audit-pipeline/scripts/audit_gate.sh
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/common.sh
source "$SCRIPT_DIR/common.sh" "$SCRIPT_DIR"

mkdir -p "$REPORT_DIR"

if [ ! -x "$AUDITOR_BIN" ]; then
  echo "⚠️ auditor 未编译，正在编译（首次约 1-2 分钟）..."
  (cd "$PIPELINE_DIR/tools/auditor" && cargo build --release)
fi

(
  cd "$PROJECT_ROOT"
  AUDITOR_CONFIG="$AUDITOR_CONFIG" AUDITOR_REPORT_DIR="$REPORT_DIR" \
    "$AUDITOR_BIN" audit --json > "$REPORT_DIR/audit_gate.json"
)

TOTAL=$(jq -r '.summary.total_violations' "$REPORT_DIR/audit_gate.json")
BLOCKER=$(jq -r '.summary.blocker_count' "$REPORT_DIR/audit_gate.json")
ERRORS=$(jq -r '.summary.error_count' "$REPORT_DIR/audit_gate.json")

echo "审计结果: total=$TOTAL blocker=$BLOCKER error=$ERRORS"

if [ "$TOTAL" -gt 0 ]; then
  echo "以下为违规明细（前 10 条，完整见 $REPORT_DIR/review_context.json）："
  jq -r '
    (.hard_violations[] | select(.count > 0) | "⛔ [" + .severity + "] " + .rule + " x" + (.count|tostring),
     (.occurrences[0:5][] | "     " + .file + ":" + (.line|tostring) + "  " + .content)),
    (.arch_violations[] | select(.count > 0) | "🏗 [" + .layer + "] " + .rule + " x" + (.count|tostring),
     (.occurrences[0:5][] | "     " + .file + ":" + (.line|tostring) + "  " + .content))
  ' "$REPORT_DIR/audit_gate.json" | head -40
  echo "❌ 审计门禁未通过：存在违规，请按行号修复后重跑"
  exit 1
fi

echo "✅ 审计门禁通过，无违规"
