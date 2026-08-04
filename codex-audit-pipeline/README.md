# codex-audit-pipeline

Codex + Rust 审计 + 日志追踪 + 模板 + Git Hooks 的**可落地版本**（修订自 DeepSeek 分享的初版配置）。

本目录是**自包含工具**，保留在仓库根的 `codex-audit-pipeline/` 下即可。所有脚本从仓库根执行，会自动定位仓库根（向上查找同时包含 `frontend/` 与 `backend/` 的目录）。

## 设计原则

1. **确定性工具优先**：正则红线扫描（auditor）、lint、编译、测试全部由确定性工具判定，不花 LLM Token。
2. **LLM 只做增量**：reviewer 只看变更文件（不是全部源码），做语义/架构审查；修复只输出 diff 补丁。
3. **判定用完整数据**：门禁读 `review_context.json`（完整），`review_context.md`（截断 4KB）仅给 LLM 展示。
4. **协议行永不截断**：`test_result.md` 最后一行固定为 `TEST_SUMMARY: PASS/FAIL/SKIP`，脚本不 truncate。

## 目录结构

```
codex-audit-pipeline/
├── .codex/
│   ├── agents/            # reviewer-rust / reviewer-angular / tester
│   ├── templates/         # Rust + Angular 生成模板
│   ├── audit.toml         # auditor 规则（SQL 写操作只允许 Repository 层等）
│   └── reports/           # 运行时产物（已 gitignore）
├── scripts/
│   ├── common.sh               # 路径自动定位（PIPELINE_DIR / PROJECT_ROOT / REPORT_DIR）
│   ├── changed_paths.sh        # 工作区 / 暂存区 / 基线 / 全量范围检测
│   ├── audit_gate.sh           # 审计门禁（完整 JSON 判定）
│   ├── generate_review_context.sh
│   ├── secret_scan.sh          # 高置信凭据扫描（不输出秘密内容）
│   ├── run_tests.sh            # 非修改式 lint/check/test/build → 报告
│   └── verify.sh               # scope → secret → audit → test 的统一入口
├── hooks/pre-commit            # 快速提交门禁（lint + 审计）
├── tools/auditor/              # Rust 审计程序（可独立编译运行）
└── README.md
```

## 前置依赖

- Rust：`cargo` + `cargo clippy`（后端）
- Node：`npx eslint` + `@angular-eslint` 配置（前端）
- 前端测试：`@angular/build:unit-test` + Vitest/jsdom
- 后端集成测试：Docker + 本地 `postgres:16-alpine`，或显式 `TEST_DATABASE_URL`
- `jq`（审计门禁解析 JSON）

## 安装

```bash
# 1. 编译审计器（已编译可跳过）
cd codex-audit-pipeline/tools/auditor && cargo build --release --locked && cd ../../..
# 2. 启用版本化的 git hooks（替换原 .git/hooks 方案）
git config core.hooksPath codex-audit-pipeline/hooks
# 3. 验证（以下命令都在仓库根执行）
bash codex-audit-pipeline/scripts/verify.sh --all
```

## 使用

```bash
# 查看工作区 / 暂存区 / 指定基线的变更范围与开关
bash codex-audit-pipeline/scripts/changed_paths.sh --working-tree
bash codex-audit-pipeline/scripts/changed_paths.sh --staged
bash codex-audit-pipeline/scripts/changed_paths.sh --base origin/main

# 审计门禁（违规即退出非 0）
bash codex-audit-pipeline/scripts/audit_gate.sh

# 只测后端 / 只测前端
RUN_RUST=true  RUN_ANGULAR=false RUN_AUDITOR=false RUN_SHELL=false bash codex-audit-pipeline/scripts/run_tests.sh
RUN_RUST=false RUN_ANGULAR=true  RUN_AUDITOR=false RUN_SHELL=false bash codex-audit-pipeline/scripts/run_tests.sh
```

目录名可覆盖：`BACKEND_DIR=server FRONTEND_DIR=web bash codex-audit-pipeline/scripts/run_tests.sh`

## 路径与环境变量

脚本通过 `scripts/common.sh` 自动定位：

- `PIPELINE_DIR`：`codex-audit-pipeline` 目录（由脚本自身位置推导）
- `PROJECT_ROOT`：仓库根（默认向上查找含 `frontend/` + `backend/` 的目录，可手动覆盖）
- `REPORT_DIR`：报告输出目录（默认 `codex-audit-pipeline/.codex/reports`）
- `AUDITOR_BIN` / `AUDITOR_CONFIG`：auditor 可执行文件与 audit.toml 路径

auditor 支持 `AUDITOR_CONFIG`（配置文件）与 `AUDITOR_REPORT_DIR`（报告目录）环境变量，默认仍为 `.codex/audit.toml` 与 `.codex/reports`（相对当前目录），便于独立使用。

## 规则说明（audit.toml）

- **硬性约束**：SQL 写操作（INSERT/UPDATE/DELETE/DROP/ALTER/TRUNCATE、`.execute(`/`.exec(`）只允许出现在 Repository / db / migrations / tests / seed 层（allowlist 放行），其余位置一律 blocker。
- **架构分层**：Service/Handler 不含 `sqlx::query`；Model 不含业务逻辑（常见 trait impl 白名单豁免）；Angular Component 不注入 `HttpClient`；Service 不操作 DOM。
- 扫描跳过 `//` 行注释（多行块注释 `/* */` 暂不处理，属已知限制）。

## 与初版相比修正了什么

| 初版问题 | 本版处理 |
| --- | --- |
| Repository 模板含 INSERT/UPDATE，与"禁止写操作"红线自相矛盾 | 规则改为"写操作只允许 Repository 层"，allowlist 放行 |
| `truncate -s 800` 可能截掉 TEST_SUMMARY 协议行 | 全部移除，协议行在文件末尾且永不截断；SKIP 分支也输出 |
| pre-commit `git add` 被 gitignore 的文件 | 钩子不再 add 报告；hooks 移入版本化目录 + `core.hooksPath` |
| `ng test --browsers=jsdom`（karma 不支持） | 明确使用 Angular unit-test builder + Vitest/jsdom，不传 Karma 参数 |
| 过时的 agent TOML schema | 移除未生效配置；当前工作流由根目录 AGENTS.md 与确定性脚本驱动 |
| reviewer 禁止读源码 → 无增量价值 | reviewer 允许读 changed_files.txt 内的变更文件，做语义审查 |
| 路由用 `git diff HEAD~1`（首次提交/未提交改动失效） | `changed_paths.sh` 基于 status/diff 合并检测 |
| `exclude_patterns` 类型错误导致 auditor 编译失败 | 已修复（Vec 直接 clone，去掉了不合法的 `unwrap_or_default`） |
| 正则误报（注释里的 INSERT、Model 的 Display/Debug impl） | 行注释跳过 + Model trait 白名单 |
| 日志取"最后一条 trace_id"可能不是出错请求 | 优先取 ERROR 日志所在 trace_id |
| 审计报告截断 4KB 导致门禁漏判 | 门禁改用完整 review_context.json |

## 已知限制

- 正则扫描是文本级，不做语法树解析：字符串字面量里的 SQL 仍可能误报；多行块注释不跳过。
- 门禁全部是非修改式检查；格式或 lint 失败后应由开发者显式修复并复核 diff。
- 本地 secret scan 仅覆盖高置信模式，仍需在 GitHub 仓库设置中启用 secret scanning 与 push protection。
- Token 消耗取决于任务复杂度，本仓库不再承诺固定估算值。
