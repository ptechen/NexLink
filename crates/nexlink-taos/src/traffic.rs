use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use taos::AsyncQueryable;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use crate::client::TaosClient;

/// A normalized traffic snapshot that can later be flushed into TDengine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficSample {
    pub ts: OffsetDateTime,
    pub peer_id: String,
    pub role: String,
    pub upload_bytes: u64,
    pub download_bytes: u64,
    pub active_connections: u32,
    pub source: String,
}

impl TrafficSample {
    pub fn table_name(&self) -> String {
        sanitize_identifier(&format!("peer_{}", self.peer_id))
    }
}

/// Repository abstraction for future traffic persistence.
#[derive(Debug, Clone)]
pub struct TrafficWriteRepository {
    client: TaosClient,
}

impl TrafficWriteRepository {
    pub fn new(client: TaosClient) -> Self {
        Self { client }
    }

    pub fn client(&self) -> &TaosClient {
        &self.client
    }

    /// Writes one traffic sample into a dedicated child table for the peer.
    ///
    /// The table is created on demand from the configured stable.
    pub async fn write_sample(&self, sample: &TrafficSample) -> Result<()> {
        self.client.ensure_traffic_schema().await?;
        let taos = self.client.connect().await?;
        let cfg = self.client.config();
        let table_name = sample.table_name();
        let timestamp = sample
            .ts
            .format(&Rfc3339)
            .context("failed to format traffic timestamp")?;
        let source = escape_string(&sample.source);
        let peer_id = escape_string(&sample.peer_id);
        let role = escape_string(&sample.role);

        let sql = format!(
            "INSERT INTO `{}.{}` USING `{}.{}` TAGS ('{}', '{}') VALUES ('{}', {}, {}, {}, '{}')",
            cfg.database,
            table_name,
            cfg.database,
            cfg.stable,
            peer_id,
            role,
            timestamp,
            sample.upload_bytes,
            sample.download_bytes,
            sample.active_connections,
            source,
        );

        taos.exec(sql)
            .await
            .context("failed to write taos traffic sample")?;

        Ok(())
    }
}

fn escape_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('\'', "\\'")
}

fn sanitize_identifier(raw: &str) -> String {
    let mut result = String::with_capacity(raw.len());
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            result.push(ch);
        } else {
            result.push('_');
        }
    }

    if result.is_empty() {
        "traffic_default".to_string()
    } else {
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    #[test]
    fn sanitizes_table_name() {
        let sample = TrafficSample {
            ts: datetime!(2026-03-19 22:18:00 UTC),
            peer_id: "12D3-Koo/Wild".to_string(),
            role: "provider".to_string(),
            upload_bytes: 1,
            download_bytes: 2,
            active_connections: 3,
            source: "relay".to_string(),
        };

        assert_eq!(sample.table_name(), "peer_12D3_Koo_Wild");
    }
}
