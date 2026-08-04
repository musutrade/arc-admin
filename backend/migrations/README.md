# Migrations

约定（已写入根目录 AGENTS.md）：

1. **迁移不可变**：一旦在任何环境应用过，就永远不修改，只新增文件。
2. 命名：`<序号>_<描述>.sql`，序号递增，例如 `0001_init_users_roles_permissions.sql`。
3. 写操作（INSERT/UPDATE/DELETE/DROP/ALTER/TRUNCATE）允许出现在 migrations 层（审计 allowlist）。

当前迁移：

| 文件 | 内容 |
| --- | --- |
| `0001_init_rbac_schema.sql` | users / roles / permission_groups / permissions / user_roles / role_permissions |
| `0002_seed_rbac_data.sql` | 5 权限组、15 个基础权限、6 个角色及初始角色-权限分配 |
| `0003_seed_admin_user.sql` | 历史演示管理员种子，仅为保持已发布迁移不可变而保留 |
| `0004_disable_default_admin.sql` | 仅当历史管理员仍使用已知密码哈希时禁用该账号 |
| `0005_add_rbac_management_permissions.sql` | 增加角色、权限目录及角色授权管理权限码，并更新内置角色授权 |

迁移文件通过 `sqlx::migrate!` 宏在**编译时**嵌入二进制。`backend/build.rs` 已声明
`migrations` 为构建输入，新增或修改迁移后，下一次 `cargo build` 会自动重新编译：

```bash
cargo build
```

后端启动后会记录数据库已成功应用的迁移数和当前二进制内嵌的迁移数。

迁移不再创建可登录的默认账号。首次部署或需要恢复管理员时，由运维人员显式执行：

```bash
BOOTSTRAP_ADMIN_PASSWORD='replace-with-a-strong-password' \
  cargo run --manifest-path backend/Cargo.toml --bin bootstrap_admin
```

密码至少 16 字符。可选变量为 `BOOTSTRAP_ADMIN_USERNAME`、
`BOOTSTRAP_ADMIN_DISPLAY_NAME` 和 `BOOTSTRAP_ADMIN_EMAIL`。命令会在事务中创建或激活账号并绑定
`super_admin`，不会输出密码；不要把密码写入 `.env`、Shell 脚本或版本库。
