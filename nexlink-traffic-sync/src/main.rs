use anyhow::{Context, Result};
use clap::Parser;
use futures_util::TryStreamExt;
use nexlink_postgresql::nexlink::peer_user::PeerUser;
use nexlink_taos::{TaosClient, TaosConfig};
use serde::Deserialize;
use taos::{AsyncFetchable, AsyncQueryable};
use tokio::time::{self, Duration};
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(
    name = "nexlink-traffic-sync",
    about = "Sync aggregated traffic from TDengine to PostgreSQL"
)]
struct Cli {
    #[arg(long, default_value_t = 30)]
    interval_secs: u64,

    #[arg(long, default_value_t = false)]
    once: bool,
}

#[derive(Debug, Deserialize)]
struct AggregatedTrafficRow {
    peer_id: String,
    send: i64,
    recv: i64,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();
    let repo = TrafficSyncService::new(TaosClient::new(TaosConfig::from_env()));

    if cli.once {
        let updated = repo.sync_once().await?;
        info!(updated, "Traffic sync finished in one-shot mode");
        return Ok(());
    }

    let mut interval = time::interval(Duration::from_secs(cli.interval_secs));
    loop {
        interval.tick().await;
        match repo.sync_once().await {
            Ok(updated) => info!(updated, "Traffic sync tick complete"),
            Err(err) => warn!(error = %err, "Traffic sync tick failed"),
        }
    }
}

struct TrafficSyncService {
    taos: TaosClient,
}

impl TrafficSyncService {
    fn new(taos: TaosClient) -> Self {
        Self { taos }
    }

    async fn sync_once(&self) -> Result<usize> {
        let rows = self.fetch_aggregated_rows().await?;
        let mut updated = 0usize;

        for row in rows {
            PeerUser::upsert_traffic_counters(&row.peer_id, row.send, row.recv)
                .await
                .with_context(|| format!("failed to upsert traffic for peer {}", row.peer_id))?;
            updated += 1;
        }

        Ok(updated)
    }

    async fn fetch_aggregated_rows(&self) -> Result<Vec<AggregatedTrafficRow>> {
        let taos = self.taos.connect().await?;
        let cfg = self.taos.config();
        taos.exec(format!("USE `{}`", cfg.database))
            .await
            .context("failed to switch taos database before traffic aggregation")?;

        let sql = format!(
            "SELECT peer_id, CAST(SUM(upload_bytes) AS BIGINT) AS send, CAST(SUM(download_bytes) AS BIGINT) AS recv FROM `{}` GROUP BY peer_id",
            cfg.stable
        );

        let mut result = taos
            .query(sql)
            .await
            .context("failed to query aggregated traffic from taos")?;

        let rows: Vec<AggregatedTrafficRow> = result
            .deserialize()
            .try_collect()
            .await
            .context("failed to deserialize aggregated taos rows")?;

        Ok(rows)
    }
}
