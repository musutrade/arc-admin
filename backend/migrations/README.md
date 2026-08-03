# Migrations

约定（已写入根目录 AGENTS.md）：

1. **迁移不可变**：一旦在任何环境应用过，就永远不修改，只新增文件。
2. 命名：`<序号>_<描述>.sql`，序号递增，例如 `0001_init_users_roles_permissions.sql`。
3. 写操作（INSERT/UPDATE/DELETE/DROP/ALTER/TRUNCATE）允许出现在 migrations 层（审计 allowlist）。

当前迁移：

| 文件 | 内容 |
| --- | --- |
| `0001_init_rbac_schema.sql` | users / roles / permission_groups / permissions / user_roles / role_permissions |
| `0002_seed_rbac_data.sql` | 5 权限组 + 15 权限 + 6 角色 + 角色-权限分配（与前端 mock 一致） |
| `0003_seed_admin_user.sql` | 管理员 admin（argon2id 哈希，密码 admin123）并绑定 super_admin |

⚠️ **重要**：迁移文件通过 `sqlx::migrate!` 宏在**编译时**嵌入二进制，新增/修改迁移后
`cargo build` 不会自动重编译（cargo 不追踪 migrations 目录）。请执行：

```bash
cargo clean -p arc-admin-backend && cargo build   # 或 touch src/main.rs 后重新 build
```
