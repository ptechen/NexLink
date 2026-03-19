use serde::{Deserialize, Serialize};

/// Runtime configuration for connecting to TDengine (taos).
///
/// `dsn` example:
/// - `taos+ws://localhost:6041/nexlink`
/// - `taos://localhost:6030/nexlink`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaosConfig {
    /// Full TDengine DSN.
    pub dsn: String,
    /// Logical database name used by NexLink traffic storage.
    pub database: String,
    /// Stable name used for traffic metrics.
    pub stable: String,
}

impl Default for TaosConfig {
    fn default() -> Self {
        Self {
            dsn: "taos+ws://localhost:6041".to_string(),
            database: "nexlink".to_string(),
            stable: "traffic_metrics".to_string(),
        }
    }
}

impl TaosConfig {
    pub fn from_env() -> Self {
        let default = Self::default();
        Self {
            dsn: std::env::var("NEXLINK_TAOS_DSN").unwrap_or(default.dsn),
            database: std::env::var("NEXLINK_TAOS_DATABASE").unwrap_or(default.database),
            stable: std::env::var("NEXLINK_TAOS_STABLE").unwrap_or(default.stable),
        }
    }
}
