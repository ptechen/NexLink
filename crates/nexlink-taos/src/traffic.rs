use anyhow::{Context, Result};
use nexlink_traffic::TrafficSnapshot;
use serde::{Deserialize, Serialize};
use taos::AsyncQueryable;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use tracing::info;

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
    pub source_ip: Option<String>,
    pub source_transport: Option<String>,
}

impl TrafficSample {
    pub fn table_name(&self) -> String {
        sanitize_identifier(&format!("peer_{}", self.peer_id))
    }

    pub fn from_snapshot(snapshot: TrafficSnapshot, ts: OffsetDateTime) -> Self {
        Self {
            ts,
            peer_id: snapshot.peer_id.to_string(),
            role: snapshot.role.unwrap_or_else(|| "unknown".to_string()),
            upload_bytes: snapshot.upload,
            download_bytes: snapshot.download,
            active_connections: snapshot.active_connections,
            source: snapshot.source.unwrap_or_else(|| "unknown".to_string()),
            source_ip: snapshot.source_ip,
            source_transport: snapshot.source_transport,
        }
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

    pub async fn flush_snapshots<I>(&self, snapshots: I, ts: OffsetDateTime) -> Result<usize>
    where
        I: IntoIterator<Item = TrafficSnapshot>,
    {
        let mut written = 0usize;
        for snapshot in snapshots {
            let sample = TrafficSample::from_snapshot(snapshot, ts);
            self.write_sample(&sample).await?;
            written += 1;
        }
        Ok(written)
    }

    /// Writes one traffic sample into a dedicated child table for the peer.
    ///
    /// The table is created on demand from the configured stable.
    pub async fn write_sample(&self, sample: &TrafficSample) -> Result<()> {
        self.client.ensure_traffic_schema().await?;
        let taos = self.client.connect().await?;
        let cfg = self.client.config();
        taos.exec(format!("USE `{}`", cfg.database))
            .await
            .context("failed to switch taos database before insert")?;
        let table_name = sample.table_name();
        let timestamp = sample
            .ts
            .format(&Rfc3339)
            .context("failed to format traffic timestamp")?;
        let source = escape_string(&sample.source);
        let peer_id = escape_string(&sample.peer_id);
        let role = escape_string(&sample.role);
        let source_ip = escape_string(sample.source_ip.as_deref().unwrap_or(""));
        let source_transport = escape_string(sample.source_transport.as_deref().unwrap_or(""));

        let sql = format!(
            "INSERT INTO `{}` USING `{}` TAGS ('{}', '{}') VALUES ('{}', {}, {}, {}, '{}', '{}', '{}')",
            table_name,
            cfg.stable,
            peer_id,
            role,
            timestamp,
            sample.upload_bytes,
            sample.download_bytes,
            sample.active_connections,
            source,
            source_ip,
            source_transport,
        );

        info!(
            table = %table_name,
            stable = %cfg.stable,
            peer_id = %sample.peer_id,
            role = %sample.role,
            upload_bytes = sample.upload_bytes,
            download_bytes = sample.download_bytes,
            active_connections = sample.active_connections,
            source = %sample.source,
            source_ip = sample.source_ip.as_deref().unwrap_or(""),
            source_transport = sample.source_transport.as_deref().unwrap_or(""),
            "Writing traffic sample to taos"
        );

        taos.exec(sql)
            .await
            .context("failed to write taos traffic sample")?;

        info!(
            table = %table_name,
            peer_id = %sample.peer_id,
            role = %sample.role,
            "Traffic sample written to taos"
        );

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
            source_ip: None,
            source_transport: None,
        };

        assert_eq!(sample.table_name(), "peer_12D3_Koo_Wild");
    }
}
