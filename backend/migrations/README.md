# Migrations

约定（写入全局 AGENTS.md）：

1. **迁移不可变**：一旦在任何环境应用过，就永远不修改，只新增文件。
2. 命名：`<序号>_<描述>.sql`，序号递增，例如 `0001_init_users_roles_permissions.sql`。
3. 写操作（INSERT/UPDATE/DELETE/DROP/ALTER/TRUNCATE）允许出现在 migrations 层（审计 allowlist）。

首个迁移示例：

```sql
-- 0001_init_users_roles_permissions.sql
CREATE TABLE users (
    id BIGSERIAL PRIMARY KEY,
    username VARCHAR(64) NOT NULL UNIQUE,
    password_hash VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at TIMESTAMPTZ
);
```

当前为空：等契约（OpenAPI + 表结构设计）定稿后再填充第一个迁移。

