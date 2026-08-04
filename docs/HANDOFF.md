# 会话交接文档（2026-08-04）

> 用途：保存当前进度，供下次会话直接续接。长期约定以 `AGENTS.md`、`docs/development.md`、`docs/architecture.md` 和 `docs/openapi.yaml` 为准。

## 当前状态

| 模块 | 已完成内容 |
| --- | --- |
| 工作流 | `cargo flow verify` 统一执行范围检测、secret scan、架构审计和测试；流程 Shell 已由 Rust CLI 替代 |
| 变更范围 | `cargo flow scope` 支持 working-tree、staged、base 和 all，并输出 backend/frontend/workflow components |
| 后端测试 | Axum 路由已抽到 library；临时 PostgreSQL 集成测试覆盖登录、鉴权和用户 CRUD |
| 契约 | OpenAPI 路由和 HTTP 方法由 Rust 测试与 `API_ROUTE_CONTRACT` 自动比对 |
| 配置安全 | 生产环境强制 32 字符以上 JWT secret 和明确 CORS origin；端口、TTL 均校验 |
| 前端 | ESLint、Prettier、19 个 Vitest 用例和 production build 纳入统一门禁；字体资源已本地化 |
| CI | GitHub Actions 拆分 quality/backend/frontend/dependency review，并上传 14 天测试报告 |
| 供应链 | Node/Rust 工具链固定，Dependabot 覆盖 npm、Cargo 和 GitHub Actions |

## 常用命令

```bash
cargo flow doctor
cargo flow verify
cargo flow verify --all
```

后端测试默认启动并销毁一次性 `postgres:16-alpine` 容器；也可导出指向隔离测试库的 `TEST_DATABASE_URL`。任何情况下都不得指向生产库。

## 下一阶段

1. 前端将 mock `DataService` 替换为基于 `HttpClient` 的真实 API、token interceptor 和错误处理。
2. 在 service 层补齐权限码校验，避免仅依赖前端隐藏按钮。
3. 接口规模明显增长后，再评估用 `utoipa` 从代码生成完整 OpenAPI，替代当前轻量路由清单检查。
4. GitHub 仓库设置中启用 `main` 分支保护、required checks、secret scanning 与 push protection。
