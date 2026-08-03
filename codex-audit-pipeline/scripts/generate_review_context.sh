#!/bin/bash
# 审计上下文生成器：产出 review_context.md（截断版，给 LLM）与 review_context.json（完整，给门禁）
# 用法（仓库根）：bash codex-audit-pipeline/scripts/generate_review_context.sh
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/common.sh
source "$SCRIPT_DIR/common.sh" "$SCRIPT_DIR"

mkdir -p "$REPORT_DIR"

if [ ! -x "$AUDITOR_BIN" ]; then
  echo "⚠️ auditor 未编译，正在编译..."
  (cd "$PIPELINE_DIR/tools/auditor" && cargo build --release)
fi

(
  cd "$PROJECT_ROOT"
  AUDITOR_CONFIG="$AUDITOR_CONFIG" AUDITOR_REPORT_DIR="$REPORT_DIR" "$AUDITOR_BIN" audit
)
echo "✅ 已生成:"
echo "   $REPORT_DIR/review_context.md  (截断版, LLM 阅读)"
echo "   $REPORT_DIR/review_context.json (完整版, 门禁判定)"
