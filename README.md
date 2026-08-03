# arc-admin

RBAC 管理后台：Angular + Angular Material 前端，Rust (Axum + SQLX) 后端。

## 目录结构

```
.
├── AGENTS.md                  # Codex 全局公约（会话启动自动加载）
├── frontend/                  # Angular 22 + Material M3 前端
│   └── src/app/{core,layout,pages}
├── backend/                   # Rust + Axum + SQLX 后端
│   ├── src/main.rs            # 入口：路由 + 连接池 + 迁移
│   ├── migrations/            # SQLX 迁移（不可变，只增不改）
│   └── .env.example
├── docs/openapi.yaml          # API 契约（前后端唯一事实来源）
├── codex-audit-pipeline/      # Codex 工作流工具（自包含）
│   ├── .codex/                # 审计规则 / 模板 / 报告产物
│   ├── scripts/               # changed_paths / audit_gate / run_tests
│   ├── hooks/                 # pre-commit（git config core.hooksPath）
│   └── tools/auditor/         # 正则审计器（Rust）
└── extracted/                 # 设计稿参考（Arco 风格 RBAC）
```

## 快速开始

```bash
# 前端
cd frontend && npm install && npm start   # http://localhost:4200

# 后端（先创建数据库，再按 .env.example 填 .env）
cp backend/.env.example backend/.env   # 修改 DATABASE_URL
cd backend && cargo run                    # http://localhost:8080/api/v1/healthz
```

## Codex 工作流

所有命令从仓库根执行，脚本会自动定位根目录：

```bash
# 1. 变更范围检测（前端/后端开关）
bash codex-audit-pipeline/scripts/changed_paths.sh

# 2. 审计门禁（auditor 全量扫描，违规即失败）
bash codex-audit-pipeline/scripts/audit_gate.sh

# 3. 测试（lint → cargo check → 两端测试）
RUN_RUST=true RUN_ANGULAR=true bash codex-audit-pipeline/scripts/run_tests.sh
```

详见 [AGENTS.md](AGENTS.md) 与 `codex-audit-pipeline/README.md`。

## API 契约

[docs/openapi.yaml](docs/openapi.yaml) 是前后端联调的唯一事实来源：

- 后端按契约实现路由与 DTO（`/api/v1/...`，服务端口由 `backend/.env` 的 `PORT` 控制，当前 8081）；
- 前端按契约替换 `DataService` 的 mock，字段映射说明见契约顶部；
- 数据库 schema 与种子数据由 `backend/migrations/` 管理（当前已应用 0001 schema + 0002 种子：5 权限组 / 15 权限 / 6 角色）。
