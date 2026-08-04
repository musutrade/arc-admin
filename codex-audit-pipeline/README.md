# arc-flow

`arc-flow` 是 arc-admin 的 Rust 开发工作流 CLI。源码位于 `codex-audit-pipeline/tools/arc-flow/`，对外命令统一为仓库根目录下的 `cargo flow`。

它替代了原先分散的 doctor、changed paths、secret scan、audit gate 和 test Shell 脚本。Git hook 仍需要一个 6 行 POSIX 启动器，因为 Git 只能执行文件；所有判断与检查都在 Rust 中完成。

## 命令

| 命令 | 用途 |
| --- | --- |
| `cargo flow doctor` | 检查工具链、环境配置、Git remote、hook 和测试数据库能力 |
| `cargo flow scope` | 列出工作区变更和选中的 backend/frontend/workflow components |
| `cargo flow secrets` | 扫描高置信凭据模式，只输出文件名 |
| `cargo flow audit` | 执行 SQL 写入位置和架构分层规则 |
| `cargo flow verify` | 按工作区范围执行 secret、audit、lint、compile、test、build |
| `cargo flow verify --all` | 执行全部 components，合并或交付前使用 |
| `cargo flow hook` | 对暂存区执行快速、非修改式 pre-commit profile |
| `cargo flow config check` | 校验 `flow.toml`、环境覆盖和强制门禁步骤 |
| `cargo flow config print --resolved` | 输出应用环境覆盖后的有效配置 |
| `cargo flow step <id>` | 在 secret 和 audit 门禁通过后单独运行一个配置步骤 |
| `cargo flow parse-logs` | 从 JSON Lines 日志提取 ERROR trace 上下文 |

所有命令支持 `--help`。结构化输出示例：

```bash
cargo flow doctor --json
cargo flow scope --json
cargo flow audit --json
```

## 范围选择

```bash
cargo flow scope                  # staged + unstaged + untracked
cargo flow scope --staged         # 仅暂存区
cargo flow scope --base origin/main
cargo flow scope --all

cargo flow verify --components backend
cargo flow verify --components frontend,workflow
```

范围结果写入：

- `.codex/reports/changed_files.txt`：供 reviewer 读取的文件清单；
- `.codex/reports/scope.json`：模式、文件和 components 的完整结构化数据。

## 工作流配置

仓库配置位于 `codex-audit-pipeline/.codex/flow.toml`。它使用带版本号的强类型 schema，配置项目路径、doctor 依赖、scope glob、临时数据库和验证命令。命令以 `program + args[]` 保存，不经过 Shell 字符串解析。

```toml
version = 1

[[steps]]
id = "backend.clippy"
label = "backend Clippy"
component = "backend"
profiles = ["full", "hook"]
program = "cargo"
args = ["clippy", "--manifest-path", "{backend}/Cargo.toml"]
cwd = "{root}"
log = "backend_clippy.log"
timeout_secs = 180
```

配置优先级为：内置安全约束 < `flow.toml` < 环境变量 < CLI 的 `--project-root` / `--config`。默认配置文件可由 `ARC_FLOW_CONFIG` 指定；路径必须位于仓库内。

支持的路径和数据库环境覆盖包括 `ARC_FLOW_BACKEND`、`ARC_FLOW_FRONTEND`、`ARC_FLOW_REPORTS`、`ARC_FLOW_TOOL_MANIFEST`、`ARC_FLOW_AUDIT_CONFIG`、`ARC_FLOW_POSTGRES_IMAGE` 和 `ARC_FLOW_DATABASE_TIMEOUT_SECS`。`REPORT_DIR`、`AUDITOR_CONFIG`、`RUST_TEST_TIMEOUT`、`ANGULAR_TEST_TIMEOUT`、`ANGULAR_BUILD_TIMEOUT` 继续兼容。

以下约束不能通过配置关闭：secret 和 audit 必须先于外部步骤；12 个基础步骤及其 component/profile 不得删除；后端测试必须使用隔离数据库；测试步骤必须检测零测试；禁止 `sh -c`；日志和配置路径不得逃出仓库。未知字段、重复 id、非法 glob/占位符和越界超时会让 `config check` 直接失败。

## 验证行为

`verify` 固定按以下顺序执行：

1. working tree 或 staged snapshot secret scan；
2. auditor 全量架构扫描；
3. 按 `flow.toml` 顺序执行选中 component/profile 的步骤。

任一确定性门禁失败时命令返回非 0。每个外部命令都有超时和独立日志，检查过程不会自动修改源码。

## 测试数据库

后端测试只接受两种数据库：

1. 显式设置的隔离 `TEST_DATABASE_URL`；
2. 由 arc-flow 按 `flow.toml` 启动并在结束时删除的临时 PostgreSQL 容器。

运行时 `DATABASE_URL` 不会被测试命令读取。Docker 不可用时，doctor 会区分“未安装”和“当前进程无 daemon 权限”。

## 报告

默认报告目录为 `codex-audit-pipeline/.codex/reports/`：

- `review_context.json`：完整审计结果，门禁判定依据；
- `review_context.md`：最多 4KB 的 LLM 上下文；
- `secret_scan.json`：扫描模式和命中文件；
- `test_result.json`：完整验证步骤、耗时和日志路径；
- `test_result.md`：人工摘要，末行固定为 `TEST_SUMMARY: PASS/FAIL`；
- `logs/*.log`：每个外部命令的原始输出。

可用 `REPORT_DIR` 覆盖报告目录，用 `AUDITOR_CONFIG` 覆盖审计规则路径；两者都必须是仓库相对路径。

## Git Hook

```bash
git config core.hooksPath codex-audit-pipeline/hooks
```

`pre-commit` 只负责进入仓库根并执行 `cargo flow hook`。hook 不跑数据库集成测试或前端 production build；完整检查由 `cargo flow verify --all` 和 CI 负责。

## 前置依赖

- Git；
- 仓库 `rust-toolchain.toml` 指定的 Rust、Cargo、rustfmt、Clippy；
- `.node-version` 指定的 Node 与 npm；
- Docker + `flow.toml` 配置的本地 PostgreSQL 镜像，或隔离的 `TEST_DATABASE_URL`。

不再依赖 jq、timeout 或 ShellCheck。

## 审计规则

`codex-audit-pipeline/.codex/audit.toml` 当前约束：

- SQL 写操作只允许在 Repository、db、migrations、tests、seed；
- Handler 和 Service 禁止直接查询 SQL；
- Angular Component 禁止直接注入 HttpClient；
- Angular Service 禁止操作 DOM；
- 模板禁止残留 actix-web 等旧框架模式。

正则审计仍是文本级检查，不替代编译器、测试或语义 reviewer。多行块注释与字符串字面量可能需要人工复核。
