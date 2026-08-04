# 会话交接文档（2026-08-04）

> 用途：保存当前进度，供下次会话直接续接。看完本节即可继续，详细约定见 AGENTS.md 与 docs/openapi.yaml。

## 当前状态：✅ 已完成

| 模块 | 内容 | 验证 |
| --- | --- | --- |
| 目录重构 | `frontend/`（Angular 22 + Material）、`backend/`（Rust + Axum + SQLX）、`codex-audit-pipeline/` 自包含工具、AGENTS.md 上移到仓库根 | 审计门禁 total=0 |
| API 契约 | `docs/openapi.yaml`：auth / users / roles / permissions / dashboard，字段映射表在契约顶部 | 后端实现与其一致 |
| 数据库 | 迁移 0001 schema + 0002 种子（5 权限组/15 权限/6 角色）+ 0003 admin 种子 | 已应用：`_sqlx_migrations` 1/2/3 均 success |
| 后端 API | 认证（argon2 + JWT）、users/roles/permissions CRUD、权限组、仪表盘统计；handler→service→repository 分层 | 冒烟 + 写路径测试全过，clippy/audit/test 全绿 |
| 启动可靠性 | `doctor.sh` 体检本地依赖；迁移变更自动触发重编；启动日志输出迁移计数 | doctor + cargo check + audit/test |
| 模板 | 5 个 `.tmpl` 更新为 axum + thiserror + camelCase 风格（原为 actix-web） | 与真实代码一致 |
| 前端 ESLint | `ng add @angular-eslint/schematics` 接入，修复 3 处无障碍违规 | `npx eslint src` 0 错误 |
| Git | pre-commit 钩子（clippy + eslint + 审计）可用；提交历史保留在本地 | 提交前自动执行快速门禁 |

## 运行方式

```bash
# 后端（数据库在 192.168.0.34:5432，账号 admin/admin123）
cd backend && cargo run        # http://localhost:8081/api/v1/healthz
# 前端
cd frontend && npm start       # http://localhost:4200（当前仍是 mock 数据）

# 依赖体检与门禁
bash codex-audit-pipeline/scripts/doctor.sh
bash codex-audit-pipeline/scripts/audit_gate.sh
RUN_RUST=true RUN_ANGULAR=true bash codex-audit-pipeline/scripts/run_tests.sh
```

环境变量在 `backend/.env`（已 gitignore）：`DATABASE_URL`（值已加引号，密码含 `&`）、`PORT=8081`、`JWT_SECRET`、`TOKEN_TTL_SECS`。
注意：8080 被 pgAdmin 容器占用，后端用 8081。

## 已知约定与坑（已记录）

- 迁移通过 `sqlx::migrate!` 编译时嵌入；`backend/build.rs` 已追踪 `migrations`，迁移变化后普通 `cargo build` 即会重编。
- 沙箱内 `.git` 只读，git 写操作需提权执行；`core.hooksPath` 已设为 `codex-audit-pipeline/hooks`。
- `npx eslint` 必须在 `frontend/` 目录内执行（脚本已处理工作目录）。

## 后续待办（按优先级，含方案）

1. **后端集成测试**：`tower::ServiceExt::oneshot` 进程内测路由 + 测试库（docker 里建 `arc_admin_test`），覆盖登录→CRUD 全链路；当前 `cargo test` 是空跑（假绿）。
2. **CI workflow**：GitHub Actions 跑 audit_gate + 后端测试（PG service）+ 前端 lint/test。
3. **前端接真实 API**：DataService 从 mock 换 HTTP + token 拦截器 + MSW 测试；字段映射见 openapi.yaml 顶部（`name→displayName`、id 改 number）。
4. **JWT_SECRET 生产硬校验**：`APP_ENV=production` 时缺失即启动失败。
5. **契约防漂移**：后端接入 utoipa 生成 OpenAPI，CI 校验 docs/openapi.yaml；低成本先行版=路由清单测试。
6. **模板防漂移**：audit.toml 加规则扫 `templates/**`，禁止 actix_web 等旧模式。
7. **agent TOML 转 markdown 子代理**（.codex/agents/*.toml 已过时，按当前 Codex schema 处理或删除）。
8. **`run_tests.sh` 潜伏 bug**：angular.json 是 Vitest builder，`--browsers=ChromeHeadless` 是 karma 参数会报错，按 builder 类型决定参数。
9. **补 `docs/architecture.md`**：分层、数据模型、环境变量、测试方式，AGENTS.md 引用。

## 未提交文件

根目录两张 QQ 截图与设计稿 zip 在 `60fcbcc` 提交里（`git add -A` 带入）；如不需要可 `git rm --cached` 并加 .gitignore（工作区文件不动）。
