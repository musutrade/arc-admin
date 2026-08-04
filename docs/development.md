# 开发指南

## 环境准备

工具链版本由仓库固定：Node 读取 `.node-version`，Rust 读取 `rust-toolchain.toml`。还需要 Git 和 Docker；若显式提供隔离的 `TEST_DATABASE_URL`，后端测试可以不使用 Docker。

```bash
cd frontend && npm ci && cd ..
cp backend/.env.example backend/.env
docker pull postgres:16-alpine
git config core.hooksPath codex-audit-pipeline/hooks
cargo flow doctor
```

运行后端前，修改不入库的 `backend/.env` 中的 `DATABASE_URL`。`APP_ENV=production` 时必须设置至少 32 字符的 `JWT_SECRET` 和明确的 `CORS_ALLOWED_ORIGINS`。

数据库迁移不会留下可登录的默认账号。首次部署先显式初始化管理员，密码至少 16 字符：

```bash
BOOTSTRAP_ADMIN_PASSWORD='replace-with-a-strong-password' \
  cargo run --manifest-path backend/Cargo.toml --bin bootstrap_admin
```

命令默认使用用户名 `admin` 和显示名 `Administrator`；可通过
`BOOTSTRAP_ADMIN_USERNAME`、`BOOTSTRAP_ADMIN_DISPLAY_NAME`、`BOOTSTRAP_ADMIN_EMAIL`
覆盖。密码只通过当前进程环境传入，不要写进 `.env`、命令脚本或提交记录。

## 日常开发

```bash
# 前端
cd frontend && npm start

# 后端
cd backend && cargo run
```

前端默认地址是 `http://localhost:4200`，开发代理会把 `/api/v1` 转发到
`http://127.0.0.1:8080`。部署时可在静态资源 `config.js` 中设置
`window.__ARC_ADMIN_CONFIG__.apiBaseUrl`，无需重新构建前端。后端端口以 `backend/.env`
的 `PORT` 为准。

- `GET /api/v1/healthz` 是进程存活检查，不访问数据库；
- `GET /api/v1/readyz` 是就绪检查，数据库不可用时返回 HTTP 503。

## 验证流程

```bash
# 查看并按未提交变更自动选择检查范围
cargo flow scope
cargo flow verify

# 合并或交付前必须全量执行
cargo flow verify --all
```

执行顺序固定为 secret scan、架构审计、格式/lint、编译、单元/集成测试、Playwright
桌面与移动端 E2E，以及前端生产构建。Rust CLI 为每一步记录耗时和独立日志；报告位于
`codex-audit-pipeline/.codex/reports/`，同时提供 JSON 和末行带 `TEST_SUMMARY` 的 Markdown。

后端集成测试仅允许使用临时容器或显式的隔离测试库：

```bash
TEST_DATABASE_URL=postgres://user:password@127.0.0.1:5432/arc_admin_test \
cargo flow verify --components backend
```

前端依赖的 CI 门禁只阻断生产依赖的 high/critical 漏洞：

```bash
cd frontend
npm audit --omit=dev --audit-level=high
```

## Git 与 GitHub

- HTTPS remote 不能包含用户名、token 或密码；优先使用 `gh auth login` 与 `gh auth setup-git` 管理凭据。
- 自动化只允许本地提交，禁止自动 push；由开发者人工执行 push 或创建 PR。
- 在 GitHub 仓库设置中保护 `main`，要求 `Quality gate`、`Backend verification`、`Frontend verification` 通过并至少一人审核。
- 启用 secret scanning、push protection 和 Dependabot alerts。公共仓库会额外运行 `Dependency review`；私有仓库是否可用取决于 GitHub 计划。
- 禁止 force push 和直接删除受保护分支。

CI 使用与本地相同的 `cargo flow` 命令，失败报告以 Actions artifact 保留 14 天。
