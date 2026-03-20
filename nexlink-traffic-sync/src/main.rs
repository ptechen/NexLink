use anyhow::{Context, Result};
use clap::Parser;
use nexlink_core::{Capability, CapabilityScope, Visibility};
use nexlink_postgresql::nexlink::peer_user::PeerUser;
use nexlink_taos::{TaosClient, TaosConfig};
use serde::Deserialize;
use taos::Timeout;
use taos::{AsAsyncConsumer, AsyncQueryable, AsyncTBuilder, IsAsyncData, TmqBuilder};
use tracing::info;
use tracing_subscriber::EnvFilter;

const DEFAULT_TOPIC: &str = "nexlink_traffic_metrics";
const LOCAL_PEER_ID: &str = "local-peer";

fn capability_descriptor(peer_id: String) -> Capability {
    Capability {
        capability_id: format!("cap:storage/traffic.sync:{peer_id}"),
        peer_id,
        bot_id: None,
        name: "storage/traffic.sync".to_string(),
        version: "1.0.0".to_string(),
        scope: CapabilityScope::Peer,
        visibility: Visibility::Private,
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "source": { "type": "string", "enum": ["taos"] },
                "target": { "type": "string", "enum": ["postgres"] }
            }
        }),
        output_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "updated_rows": { "type": "integer" }
            }
        }),
        metadata: serde_json::json!({
            "source": "taos",
            "sink": "postgres",
            "mode": "tmq",
            "service": "nexlink-traffic-sync"
        }),
        enabled: true,
    }
}

#[derive(Parser, Debug)]
#[command(
    name = "nexlink-traffic-sync",
    about = "Consume TDengine TMQ traffic topic and update PostgreSQL"
)]
struct Cli {
    #[arg(long, default_value = DEFAULT_TOPIC)]
    topic: String,

    #[arg(long, default_value = "nexlink-traffic-sync")]
    group_id: String,

    #[arg(long, default_value_t = false)]
    init_topic: bool,
}

#[derive(Debug, Deserialize)]
struct TrafficRow {
    peer_id: String,
    upload_bytes: i64,
    download_bytes: i64,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();
    let taos_cfg = TaosConfig::from_env();
    let service = TrafficSyncService::new(TaosClient::new(taos_cfg));
    let descriptor = capability_descriptor(LOCAL_PEER_ID.to_string());
    info!(capability = ?descriptor, "traffic sync capability descriptor");

    if cli.init_topic {
        service.ensure_topic(&cli.topic).await?;
        info!(topic = %cli.topic, "TMQ topic initialized");
    }

    service.consume_loop(&cli.topic, &cli.group_id).await
}

struct TrafficSyncService {
    taos: TaosClient,
}

impl TrafficSyncService {
    fn new(taos: TaosClient) -> Self {
        Self { taos }
    }

    async fn ensure_topic(&self, topic: &str) -> Result<()> {
        self.taos.ensure_traffic_schema().await?;
        let taos = self.taos.connect().await?;
        let cfg = self.taos.config();
        taos.exec(format!("USE `{}`", cfg.database)).await?;
        taos.exec(format!("DROP TOPIC IF EXISTS `{}`", topic))
            .await?;
        taos.exec(format!(
            "CREATE TOPIC `{}` AS SELECT peer_id, upload_bytes, download_bytes FROM `{}`",
            topic, cfg.stable
        ))
        .await
        .with_context(|| format!("failed to create tmq topic {}", topic))?;
        Ok(())
    }

    async fn consume_loop(&self, topic: &str, group_id: &str) -> Result<()> {
        let dsn = self.tmq_dsn(group_id)?;
        let tmq = TmqBuilder::from_dsn(&dsn).context("failed to build tmq dsn")?;
        let mut consumer = tmq.build().await.context("failed to create tmq consumer")?;
        consumer
            .subscribe([topic])
            .await
            .context("failed to subscribe tmq topic")?;
        info!(topic = %topic, group_id = %group_id, "TMQ consumer started");

        loop {
            let maybe = consumer
                .recv_timeout(Timeout::Never)
                .await
                .context("tmq recv failed")?;
            let Some((offset, message)) = maybe else {
                info!("TMQ recv returned no message, continue waiting");
                continue;
            };

            if let Some(data) = message.into_data() {
                loop {
                    let maybe_block: Option<taos::RawBlock> = data
                        .fetch_raw_block()
                        .await
                        .context("failed to fetch tmq block")?;
                    let Some(block) = maybe_block else {
                        break;
                    };

                    let rows: Vec<TrafficRow> = block
                        .deserialize()
                        .collect::<std::result::Result<Vec<_>, _>>()
                        .context("failed to deserialize tmq rows")?;

                    let mut total_up = 0i64;
                    let mut total_down = 0i64;
                    let mut peer_id = String::new();
                    for row in rows {
                        if peer_id.is_empty() {
                            peer_id = row.peer_id;
                        }
                        total_up += row.upload_bytes;
                        total_down += row.download_bytes;
                    }

                    if !peer_id.is_empty() && (total_up != 0 || total_down != 0) {
                        PeerUser::add_traffic_delta(&peer_id, total_up, total_down)
                            .await
                            .with_context(|| {
                                format!("failed to update postgres for peer {}", peer_id)
                            })?;
                        info!(peer_id = %peer_id, send_delta = total_up, recv_delta = total_down, "Applied TMQ traffic delta to postgres");
                    }
                }
            } else {
                info!("Skipping non-data TMQ message");
            }

            consumer
                .commit(offset)
                .await
                .context("failed to commit tmq offset")?;
        }
    }

    fn tmq_dsn(&self, group_id: &str) -> Result<String> {
        let cfg = self.taos.config();
        let raw = cfg.dsn.trim();
        let base = if let Some(rest) = raw.strip_prefix("taos+ws://") {
            format!("taos://{}", rest)
        } else if let Some(rest) = raw.strip_prefix("ws://") {
            format!("taos://{}", rest)
        } else if let Some(rest) = raw.strip_prefix("http://") {
            format!("taos://{}", rest)
        } else {
            raw.to_string()
        };
        let sep = if base.contains('?') { '&' } else { '?' };
        Ok(format!("{}{sep}group.id={}", base, group_id))
    }
}
