#!/bin/bash
# 审计上下文生成器：产出 review_context.md（截断版，给 LLM）与 review_context.json（完整，给门禁）
# 用法（仓库根）：bash codex-audit-pipeline/scripts/generate_review_context.sh
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=/dev/null
source "$SCRIPT_DIR/common.sh" "$SCRIPT_DIR"

mkdir -p "$REPORT_DIR"

(cd "$PIPELINE_DIR/tools/auditor" && cargo build --release --locked --quiet)

(
  cd "$PROJECT_ROOT"
  AUDITOR_CONFIG="$AUDITOR_CONFIG" AUDITOR_REPORT_DIR="$REPORT_DIR" "$AUDITOR_BIN" audit
)
echo "✅ 已生成:"
echo "   $REPORT_DIR/review_context.md  (截断版, LLM 阅读)"
echo "   $REPORT_DIR/review_context.json (完整版, 门禁判定)"
