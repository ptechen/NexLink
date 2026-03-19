pub mod credentials;
pub mod http_connect;
pub mod provider_handler;
pub mod socks5;
pub mod unified_proxy;

use libp2p::StreamProtocol;
use serde::{Deserialize, Serialize};

pub const PROXY_PROTOCOL: StreamProtocol = StreamProtocol::new("/nexlink/proxy/1.0.0");
pub const CREDENTIALS_PROTOCOL: StreamProtocol = StreamProtocol::new("/nexlink/credentials/1.0.0");
pub const CREDENTIALS_SYNC_PROTOCOL: StreamProtocol =
    StreamProtocol::new("/nexlink/credentials-sync/1.0.0");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyCredentials {
    pub username: String,
    pub password: String,
}
