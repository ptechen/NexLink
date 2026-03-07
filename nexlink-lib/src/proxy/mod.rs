pub mod provider_handler;
pub mod http_connect;
pub mod socks5;
pub mod unified_proxy;

use libp2p::StreamProtocol;

pub const PROXY_PROTOCOL: StreamProtocol = StreamProtocol::new("/nexlink/proxy/1.0.0");
