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

## 日常开发

```bash
# 前端
cd frontend && npm start

# 后端
cd backend && cargo run
```

前端默认地址是 `http://localhost:4200`。后端默认健康检查是 `http://localhost:8080/api/v1/healthz`；端口以 `backend/.env` 的 `PORT` 为准。

## 验证流程

```bash
# 查看并按未提交变更自动选择检查范围
cargo flow scope
cargo flow verify

# 合并或交付前必须全量执行
cargo flow verify --all
```

执行顺序固定为 secret scan、架构审计、格式/lint、编译、测试和前端生产构建。Rust CLI 为每一步记录耗时和独立日志；报告位于 `codex-audit-pipeline/.codex/reports/`，同时提供 JSON 和末行带 `TEST_SUMMARY` 的 Markdown。

后端集成测试仅允许使用临时容器或显式的隔离测试库：

```bash
TEST_DATABASE_URL=postgres://user:password@127.0.0.1:5432/arc_admin_test \
cargo flow verify --components backend
```

## Git 与 GitHub

- HTTPS remote 不能包含用户名、token 或密码；优先使用 `gh auth login` 与 `gh auth setup-git` 管理凭据。
- 自动化只允许本地提交，禁止自动 push；由开发者人工执行 push 或创建 PR。
- 在 GitHub 仓库设置中保护 `main`，要求 `Quality gate`、`Backend verification`、`Frontend verification` 通过并至少一人审核。
- 启用 secret scanning、push protection 和 Dependabot alerts。公共仓库会额外运行 `Dependency review`；私有仓库是否可用取决于 GitHub 计划。
- 禁止 force push 和直接删除受保护分支。

CI 使用与本地相同的 `cargo flow` 命令，失败报告以 Actions artifact 保留 14 天。
