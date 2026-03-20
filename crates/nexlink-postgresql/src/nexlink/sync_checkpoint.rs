use super::POSTGRES_POOL;
use super::Result;

pub struct SyncCheckpoint;

impl SyncCheckpoint {
    pub async fn ensure_table() -> Result<()> {
        let sql = r#"
            CREATE TABLE IF NOT EXISTS sync_checkpoint (
                service_name VARCHAR(128) PRIMARY KEY,
                last_ts      TIMESTAMPTZ,
                updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
        "#;
        let mut pool = POSTGRES_POOL.acquire().await?;
        sqlx::query(sql).execute(&mut *pool).await?;
        Ok(())
    }

    pub async fn get_last_ts(service_name: &str) -> Result<Option<time::OffsetDateTime>> {
        Self::ensure_table().await?;
        let sql = "SELECT last_ts FROM sync_checkpoint WHERE service_name = $1";
        let mut pool = POSTGRES_POOL.acquire().await?;
        sqlx::query_scalar::<_, Option<time::OffsetDateTime>>(sql)
            .bind(service_name)
            .fetch_optional(&mut *pool)
            .await
            .map(|row| row.flatten())
    }

    pub async fn set_last_ts(service_name: &str, last_ts: time::OffsetDateTime) -> Result<()> {
        Self::ensure_table().await?;
        let sql = r#"
            INSERT INTO sync_checkpoint (service_name, last_ts)
            VALUES ($1, $2)
            ON CONFLICT (service_name) DO UPDATE
            SET last_ts = EXCLUDED.last_ts,
                updated_at = NOW()
        "#;
        let mut pool = POSTGRES_POOL.acquire().await?;
        sqlx::query(sql)
            .bind(service_name)
            .bind(last_ts)
            .execute(&mut *pool)
            .await?;
        Ok(())
    }
}
