# arc-admin

RBAC 管理后台：Angular + Angular Material 前端，Rust (Axum + SQLX) 后端。

## 目录结构

```
.
├── AGENTS.md                  # Codex 全局公约（会话启动自动加载）
├── frontend/                  # Angular 22 + Material M3 前端
│   └── src/app/{core,layout,pages}
├── backend/                   # Rust + Axum + SQLX 后端
│   ├── src/lib.rs             # 应用路由与可测试的 AppState
│   ├── src/main.rs            # 配置、连接池、迁移与监听入口
│   ├── migrations/            # SQLX 迁移（不可变，只增不改）
│   └── .env.example
├── docs/openapi.yaml          # API 契约（前后端唯一事实来源）
├── .arc-flow/                 # 可复用工作流 schema v2 配置
├── codex-audit-pipeline/      # Codex 工作流工具（自包含）
│   ├── .codex/                # 审计规则 / 模板 / 报告产物
│   ├── hooks/                 # pre-commit（仅启动 arc-flow）
│   └── tools/arc-flow/        # Rust 工作流 CLI
└── .github/                   # CI 与 Dependabot 配置
```

## 快速开始

```bash
# 一次性准备
cd frontend && npm ci && cd ..
cp backend/.env.example backend/.env       # 修改 DATABASE_URL
docker pull postgres:16-alpine
git config core.hooksPath codex-audit-pipeline/hooks
cargo flow doctor

# 前端（新终端）
cd frontend && npm start                  # http://localhost:4200

# 后端（先创建 DATABASE_URL 指向的本地数据库）
cd backend && cargo run                    # http://localhost:8080/api/v1/healthz
```

首次运行或环境变化后，可从仓库根执行依赖体检（不会连接数据库）：

```bash
cargo flow doctor
```

## Codex 工作流

所有命令从仓库根执行，`cargo flow` 会定位项目并写入结构化报告：

```bash
# 查看范围并按工作区变更自动验证
cargo flow scope
cargo flow verify

# 校验或查看应用环境覆盖后的工作流配置
cargo flow config check
cargo flow config print --resolved

# 合并前完整验证（secret scan → audit → lint/check/test/build）
cargo flow verify --all
```

详见 [开发指南](docs/development.md)、[架构说明](docs/architecture.md)、[AGENTS.md](AGENTS.md) 与 [工作流说明](codex-audit-pipeline/README.md)。

## API 契约

[docs/openapi.yaml](docs/openapi.yaml) 是前后端联调的唯一事实来源：

- 后端按契约实现路由与 DTO（`/api/v1/...`，服务端口由 `backend/.env` 的 `PORT` 控制，默认 8080）；
- 前端按契约替换 `DataService` 的 mock，字段映射说明见契约顶部；
- 数据库 schema 与种子数据由 `backend/migrations/` 管理（0001 schema、0002 RBAC 种子、0003 本地管理员种子）。
- `backend/tests/openapi_contract.rs` 会在 CI 中校验 OpenAPI 路径和 HTTP 方法没有漂移。
