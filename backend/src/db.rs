//! 数据库层：连接池初始化、迁移与健康检查（SQL 允许出现在 db 层，见审计 allowlist）

use sqlx::migrate::Migrator;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::time::Duration;

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

pub struct MigrationStatus {
    pub applied: i64,
    pub embedded: usize,
}

pub async fn init_pool(database_url: &str) -> anyhow::Result<PgPool> {
    Ok(PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(5))
        .connect(database_url)
        .await?)
}

pub async fn run_migrations(pool: &PgPool) -> anyhow::Result<MigrationStatus> {
    MIGRATOR.run(pool).await?;

    let applied =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM _sqlx_migrations WHERE success = TRUE")
            .fetch_one(pool)
            .await?;

    Ok(MigrationStatus {
        applied,
        embedded: MIGRATOR
            .iter()
            .filter(|migration| !migration.migration_type.is_down_migration())
            .count(),
    })
}

pub async fn ping(pool: &PgPool) -> bool {
    sqlx::query("SELECT 1").execute(pool).await.is_ok()
}
