#!/bin/bash
# 变更范围检测：输出变更文件清单 + RUN_RUST / RUN_ANGULAR 开关
# 用法（仓库根）：bash codex-audit-pipeline/scripts/changed_paths.sh
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/common.sh
source "$SCRIPT_DIR/common.sh" "$SCRIPT_DIR"

mkdir -p "$REPORT_DIR"
CHANGED_FILE="$REPORT_DIR/changed_files.txt"

cd "$PROJECT_ROOT"

BACKEND_DIR="${BACKEND_DIR:-backend}"
FRONTEND_DIR="${FRONTEND_DIR:-frontend}"

if ! git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  echo "⚠️ 当前目录不是 git 仓库，无法检测变更范围。"
  echo "   请先执行: git init && git add -A && git commit -m \"chore: init\""
  echo "RUN_RUST=false"
  echo "RUN_ANGULAR=false"
  exit 0
fi

# 合并：已暂存 + 未暂存 + 未跟踪（排除被忽略文件）
{
  git diff --name-only
  git diff --cached --name-only
  git ls-files --others --exclude-standard
} | sort -u | grep -v '^$' > "$CHANGED_FILE"

if [ ! -s "$CHANGED_FILE" ]; then
  echo "⚠️ 未检测到任何变更（工作区干净）"
  echo "RUN_RUST=false"
  echo "RUN_ANGULAR=false"
  exit 0
fi

RUN_RUST=false
RUN_ANGULAR=false
if grep -qE "^${BACKEND_DIR}/|^codex-audit-pipeline/tools/|^codex-audit-pipeline/scripts/" "$CHANGED_FILE"; then
  RUN_RUST=true
fi
if grep -qE "^${FRONTEND_DIR}/|^src/" "$CHANGED_FILE"; then
  RUN_ANGULAR=true
fi

echo "变更文件清单: $CHANGED_FILE ($(wc -l < "$CHANGED_FILE") 个文件)"
echo "RUN_RUST=$RUN_RUST"
echo "RUN_ANGULAR=$RUN_ANGULAR"
