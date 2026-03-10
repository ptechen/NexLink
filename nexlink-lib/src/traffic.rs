use copy_bidirectional::copy_bidirectional::{TrafficTrait, copy_bidirectional};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncWrite};

#[derive(Debug, Clone, Default)]
pub struct TrafficCounter {
    pub bytes_sent: Arc<AtomicU64>,
    pub bytes_received: Arc<AtomicU64>,
    pub active_connections: Arc<AtomicU32>,
}

impl TrafficCounter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_sent(&self, n: u64) {
        self.bytes_sent.fetch_add(n, Ordering::Relaxed);
    }

    pub fn add_received(&self, n: u64) {
        self.bytes_received.fetch_add(n, Ordering::Relaxed);
    }

    pub fn inc_connections(&self) {
        self.active_connections.fetch_add(1, Ordering::Relaxed);
    }

    pub fn dec_connections(&self) {
        self.active_connections.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> TrafficSnapshot {
        TrafficSnapshot {
            bytes_sent: self.bytes_sent.load(Ordering::Relaxed),
            bytes_received: self.bytes_received.load(Ordering::Relaxed),
            active_connections: self.active_connections.load(Ordering::Relaxed),
        }
    }
}

pub struct TrafficSnapshot {
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub active_connections: u32,
}

impl TrafficTrait for TrafficCounter {
    fn add(info: &Arc<Self>, size: u64, is_upload: bool) {
        if is_upload {
            info.bytes_sent.fetch_add(size, Ordering::Relaxed);
        } else {
            info.bytes_received.fetch_add(size, Ordering::Relaxed);
        }
    }
}

struct NoTraffic;

impl TrafficTrait for NoTraffic {
    fn add(_info: &Arc<Self>, _size: u64, _is_upload: bool) {}
}

pub async fn relay_bidirectional<A, B>(
    a: &mut A,
    b: &mut B,
    traffic: Option<&TrafficCounter>,
) -> tokio::io::Result<()>
where
    A: AsyncRead + AsyncWrite + Unpin + ?Sized,
    B: AsyncRead + AsyncWrite + Unpin + ?Sized,
{
    let exit_flag = false;
    let mut times = 0u64;

    if let Some(traffic) = traffic {
        let traffic = Arc::new(traffic.clone());
        copy_bidirectional(a, b, &traffic, &exit_flag, &mut times).await
    } else {
        let traffic = Arc::new(NoTraffic);
        copy_bidirectional(a, b, &traffic, &exit_flag, &mut times).await
    }
}

/// Copy data from reader to writer, counting bytes transferred.
pub async fn counted_copy<R, W>(
    reader: &mut R,
    writer: &mut W,
    counter: &AtomicU64,
) -> tokio::io::Result<u64>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut buf = [0u8; 8192];
    let mut total = 0u64;
    loop {
        let n = reader.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        writer.write_all(&buf[..n]).await?;
        counter.fetch_add(n as u64, Ordering::Relaxed);
        total += n as u64;
    }
    Ok(total)
}
