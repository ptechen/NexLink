use anyhow::{Context, Result};
use taos::{AsyncQueryable, AsyncTBuilder, Taos, TaosBuilder};

use crate::config::TaosConfig;

/// Lightweight TDengine client wrapper used by NexLink infrastructure crates.
#[derive(Debug, Clone)]
pub struct TaosClient {
    config: TaosConfig,
}

impl TaosClient {
    pub fn new(config: TaosConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &TaosConfig {
        &self.config
    }

    pub async fn connect(&self) -> Result<Taos> {
        TaosBuilder::from_dsn(self.config.dsn.as_str())
            .context("failed to parse taos dsn")?
            .build()
            .await
            .context("failed to build taos client")
    }

    /// Create the target database and the traffic stable if they do not already exist.
    pub async fn ensure_traffic_schema(&self) -> Result<()> {
        let taos = self.connect().await?;

        taos.exec(format!(
            "CREATE DATABASE IF NOT EXISTS `{}` KEEP 3650",
            self.config.database
        ))
        .await
        .context("failed to create taos database")?;

        taos.exec(format!(
            "CREATE STABLE IF NOT EXISTS `{}.{}` (
                `ts` TIMESTAMP,
                `upload_bytes` BIGINT,
                `download_bytes` BIGINT,
                `active_connections` INT,
                `source` BINARY(32),
                `source_ip` BINARY(64),
                `source_transport` BINARY(32)
            ) TAGS (
                `peer_id` BINARY(96),
                `role` BINARY(16)
            )",
            self.config.database, self.config.stable
        ))
        .await
        .context("failed to create taos traffic stable")?;

        Ok(())
    }
}
