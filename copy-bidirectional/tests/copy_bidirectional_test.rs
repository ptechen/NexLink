use copy_bidirectional::copy_bidirectional::{TrafficTrait, copy_bidirectional};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// 简单的流量统计实现
#[derive(Clone)]
struct TrafficCounter {
    upload: Arc<AtomicU64>,
    download: Arc<AtomicU64>,
}

impl TrafficCounter {
    fn new() -> Self {
        Self {
            upload: Arc::new(AtomicU64::new(0)),
            download: Arc::new(AtomicU64::new(0)),
        }
    }

    fn get_upload(&self) -> u64 {
        self.upload.load(Ordering::Relaxed)
    }

    fn get_download(&self) -> u64 {
        self.download.load(Ordering::Relaxed)
    }

    fn get_total(&self) -> u64 {
        self.get_upload() + self.get_download()
    }
}

impl TrafficTrait for TrafficCounter {
    fn add(info: &Arc<Self>, size: u64, is_upload: bool) {
        if is_upload {
            info.upload.fetch_add(size, Ordering::Relaxed);
        } else {
            info.download.fetch_add(size, Ordering::Relaxed);
        }
    }
}

/// 无操作的流量统计（用于不需要统计的测试）
struct NoOpTraffic;

impl TrafficTrait for NoOpTraffic {
    fn add(_info: &Arc<Self>, _size: u64, _is_upload: bool) {}
}

#[tokio::test]
async fn test_copy_bidirectional_basic() {
    let (mut client, mut server) = tokio::io::duplex(1024);

    let _traffic = Arc::new(TrafficCounter::new());
    let _traffic_clone = Arc::clone(&_traffic);
    let _exit_flag = false;
    let mut _times = 0u64;

    // 服务器任务：接收数据并回显
    let server_task = tokio::spawn(async move {
        let mut buf = vec![0u8; 1024];
        let n = server.read(&mut buf).await.unwrap();
        assert_eq!(&buf[..n], b"Hello, World!");

        server.write_all(b"Echo: Hello, World!").await.unwrap();
        server.shutdown().await.unwrap();
    });

    // 客户端任务：发送数据
    let client_task = tokio::spawn(async move {
        client.write_all(b"Hello, World!").await.unwrap();

        let mut response = String::new();
        client.read_to_string(&mut response).await.unwrap();
        assert_eq!(response, "Echo: Hello, World!");
    });

    server_task.await.unwrap();
    client_task.await.unwrap();

    // 注意：这个基本测试没有使用 copy_bidirectional，
    // 它只是验证了测试框架的设置
}

#[tokio::test]
async fn test_copy_bidirectional_with_traffic_counting() {
    let (mut a_client, mut a_server) = tokio::io::duplex(4096);
    let (mut b_client, mut b_server) = tokio::io::duplex(4096);

    let traffic = Arc::new(TrafficCounter::new());
    let traffic_clone = Arc::clone(&traffic);
    let exit_flag = false;
    let mut times = 0u64;

    // A 端任务
    let a_task = tokio::spawn(async move {
        a_client.write_all(b"Message from A").await.unwrap();
        a_client.shutdown().await.unwrap();

        let mut buf = Vec::new();
        a_client.read_to_end(&mut buf).await.unwrap();
        buf
    });

    // B 端任务
    let b_task = tokio::spawn(async move {
        b_client.write_all(b"Message from B").await.unwrap();
        b_client.shutdown().await.unwrap();

        let mut buf = Vec::new();
        b_client.read_to_end(&mut buf).await.unwrap();
        buf
    });

    // 双向复制任务
    let copy_task = tokio::spawn(async move {
        copy_bidirectional(
            &mut a_server,
            &mut b_server,
            &traffic_clone,
            &exit_flag,
            &mut times,
        )
        .await
    });

    let a_result = a_task.await.unwrap();
    let b_result = b_task.await.unwrap();
    copy_task.await.unwrap().unwrap();

    // 验证数据正确传输
    assert_eq!(a_result, b"Message from B");
    assert_eq!(b_result, b"Message from A");

    // 验证流量统计
    let total_traffic = traffic.get_total();
    assert_eq!(
        total_traffic,
        b"Message from A".len() as u64 + b"Message from B".len() as u64
    );
}

#[tokio::test]
async fn test_copy_bidirectional_large_data() {
    let (mut a_client, mut a_server) = tokio::io::duplex(65536);
    let (mut b_client, mut b_server) = tokio::io::duplex(65536);

    let traffic = Arc::new(TrafficCounter::new());
    let traffic_clone = Arc::clone(&traffic);
    let exit_flag = false;
    let mut times = 0u64;

    // 创建大数据块 (1MB)
    let data_size = 1024 * 1024;
    let large_data: Vec<u8> = (0..data_size).map(|i| (i % 256) as u8).collect();
    let large_data_clone = large_data.clone();

    // A 端发送大数据
    let a_task = tokio::spawn(async move {
        a_client.write_all(&large_data).await.unwrap();
        a_client.shutdown().await.unwrap();

        let mut buf = Vec::new();
        a_client.read_to_end(&mut buf).await.unwrap();
        buf
    });

    // B 端接收并回显
    let b_task = tokio::spawn(async move {
        let mut buf = Vec::new();
        b_client.read_to_end(&mut buf).await.unwrap();
        assert_eq!(buf.len(), data_size);
        buf
    });

    // 双向复制
    let copy_task = tokio::spawn(async move {
        copy_bidirectional(
            &mut a_server,
            &mut b_server,
            &traffic_clone,
            &exit_flag,
            &mut times,
        )
        .await
    });

    let b_result = b_task.await.unwrap();
    a_task.await.unwrap();
    copy_task.await.unwrap().unwrap();

    // 验证数据完整性
    assert_eq!(b_result, large_data_clone);

    // 验证流量统计
    assert!(traffic.get_total() >= data_size as u64);
}

#[tokio::test]
async fn test_copy_bidirectional_concurrent_writes() {
    let (mut a_client, mut a_server) = tokio::io::duplex(8192);
    let (mut b_client, mut b_server) = tokio::io::duplex(8192);

    let traffic = Arc::new(NoOpTraffic);
    let exit_flag = false;
    let mut times = 0u64;

    // A 端发送多条消息
    let a_task = tokio::spawn(async move {
        for i in 0..10 {
            let msg = format!("A-{}", i);
            a_client.write_all(msg.as_bytes()).await.unwrap();
        }
        a_client.shutdown().await.unwrap();

        let mut buf = Vec::new();
        a_client.read_to_end(&mut buf).await.unwrap();
        buf
    });

    // B 端发送多条消息
    let b_task = tokio::spawn(async move {
        for i in 0..10 {
            let msg = format!("B-{}", i);
            b_client.write_all(msg.as_bytes()).await.unwrap();
        }
        b_client.shutdown().await.unwrap();

        let mut buf = Vec::new();
        b_client.read_to_end(&mut buf).await.unwrap();
        buf
    });

    // 双向复制
    let copy_task = tokio::spawn(async move {
        copy_bidirectional(
            &mut a_server,
            &mut b_server,
            &traffic,
            &exit_flag,
            &mut times,
        )
        .await
    });

    let a_result = a_task.await.unwrap();
    let b_result = b_task.await.unwrap();
    copy_task.await.unwrap().unwrap();

    // 验证接收到数据
    assert!(!a_result.is_empty());
    assert!(!b_result.is_empty());
}

#[tokio::test]
async fn test_copy_bidirectional_empty_stream() {
    let (mut a_client, mut a_server) = tokio::io::duplex(1024);
    let (mut b_client, mut b_server) = tokio::io::duplex(1024);

    let traffic = Arc::new(TrafficCounter::new());
    let traffic_clone = Arc::clone(&traffic);
    let exit_flag = false;
    let mut times = 0u64;

    // A 端立即关闭
    let a_task = tokio::spawn(async move {
        a_client.shutdown().await.unwrap();
    });

    // B 端立即关闭
    let b_task = tokio::spawn(async move {
        b_client.shutdown().await.unwrap();
    });

    // 双向复制
    let copy_task = tokio::spawn(async move {
        copy_bidirectional(
            &mut a_server,
            &mut b_server,
            &traffic_clone,
            &exit_flag,
            &mut times,
        )
        .await
    });

    a_task.await.unwrap();
    b_task.await.unwrap();
    copy_task.await.unwrap().unwrap();

    // 验证没有流量
    assert_eq!(traffic.get_total(), 0);
    assert_eq!(times, 0);
}

#[tokio::test]
async fn test_copy_bidirectional_one_way() {
    let (mut a_client, mut a_server) = tokio::io::duplex(4096);
    let (mut b_client, mut b_server) = tokio::io::duplex(4096);

    let traffic = Arc::new(TrafficCounter::new());
    let traffic_clone = Arc::clone(&traffic);
    let exit_flag = false;
    let mut times = 0u64;

    // A 端只发送数据
    let a_task = tokio::spawn(async move {
        a_client.write_all(b"One way message").await.unwrap();
        a_client.shutdown().await.unwrap();
    });

    // B 端只接收数据
    let b_task = tokio::spawn(async move {
        let mut buf = Vec::new();
        b_client.read_to_end(&mut buf).await.unwrap();
        buf
    });

    // 双向复制
    let copy_task = tokio::spawn(async move {
        copy_bidirectional(
            &mut a_server,
            &mut b_server,
            &traffic_clone,
            &exit_flag,
            &mut times,
        )
        .await
    });

    a_task.await.unwrap();
    let b_result = b_task.await.unwrap();
    copy_task.await.unwrap().unwrap();

    // 验证单向传输
    assert_eq!(b_result, b"One way message");
    assert_eq!(traffic.get_total(), b"One way message".len() as u64);
}

#[tokio::test]
async fn test_traffic_counter_separate_directions() {
    let (mut a_client, mut a_server) = tokio::io::duplex(4096);
    let (mut b_client, mut b_server) = tokio::io::duplex(4096);

    let traffic = Arc::new(TrafficCounter::new());
    let traffic_clone = Arc::clone(&traffic);
    let exit_flag = false;
    let mut times = 0u64;

    // A 端发送较小数据
    let a_data = b"Small";
    let a_task = tokio::spawn(async move {
        a_client.write_all(a_data).await.unwrap();
        a_client.shutdown().await.unwrap();

        let mut buf = Vec::new();
        a_client.read_to_end(&mut buf).await.unwrap();
        buf
    });

    // B 端发送较大数据
    let b_data = b"Much larger message from B";
    let b_task = tokio::spawn(async move {
        b_client.write_all(b_data).await.unwrap();
        b_client.shutdown().await.unwrap();

        let mut buf = Vec::new();
        b_client.read_to_end(&mut buf).await.unwrap();
        buf
    });

    // 双向复制
    let copy_task = tokio::spawn(async move {
        copy_bidirectional(
            &mut a_server,
            &mut b_server,
            &traffic_clone,
            &exit_flag,
            &mut times,
        )
        .await
    });

    let a_result = a_task.await.unwrap();
    let b_result = b_task.await.unwrap();
    copy_task.await.unwrap().unwrap();

    // 验证双向数据
    assert_eq!(a_result, b"Much larger message from B");
    assert_eq!(b_result, b"Small");

    // 验证总流量
    let expected_total = (b"Small".len() + b"Much larger message from B".len()) as u64;
    assert_eq!(traffic.get_total(), expected_total);
}

#[tokio::test]
async fn test_times_counter() {
    let (mut a_client, mut a_server) = tokio::io::duplex(4096);
    let (mut b_client, mut b_server) = tokio::io::duplex(4096);

    let traffic = Arc::new(NoOpTraffic);
    let exit_flag = false;
    let mut times = 0u64;

    // 发送一些数据
    let a_task = tokio::spawn(async move {
        a_client.write_all(b"Test").await.unwrap();
        a_client.shutdown().await.unwrap();
    });

    let b_task = tokio::spawn(async move {
        let mut buf = Vec::new();
        b_client.read_to_end(&mut buf).await.unwrap();
    });

    // 双向复制
    copy_bidirectional(
        &mut a_server,
        &mut b_server,
        &traffic,
        &exit_flag,
        &mut times,
    )
    .await
    .unwrap();

    a_task.await.unwrap();
    b_task.await.unwrap();

    // 验证 times 计数器被更新
    assert!(times > 0, "Times counter should be incremented");
}

#[test]
fn test_traffic_counter_creation() {
    let counter = TrafficCounter::new();
    assert_eq!(counter.get_upload(), 0);
    assert_eq!(counter.get_download(), 0);
    assert_eq!(counter.get_total(), 0);
}

#[test]
fn test_traffic_counter_manual_add() {
    let counter = Arc::new(TrafficCounter::new());

    // 添加上传流量
    TrafficCounter::add(&counter, 100, true);
    assert_eq!(counter.get_upload(), 100);
    assert_eq!(counter.get_download(), 0);
    assert_eq!(counter.get_total(), 100);

    // 添加下载流量
    TrafficCounter::add(&counter, 200, false);
    assert_eq!(counter.get_upload(), 100);
    assert_eq!(counter.get_download(), 200);
    assert_eq!(counter.get_total(), 300);

    // 再次添加
    TrafficCounter::add(&counter, 50, true);
    TrafficCounter::add(&counter, 75, false);
    assert_eq!(counter.get_upload(), 150);
    assert_eq!(counter.get_download(), 275);
    assert_eq!(counter.get_total(), 425);
}

#[test]
fn test_traffic_counter_clone() {
    let counter = Arc::new(TrafficCounter::new());
    TrafficCounter::add(&counter, 100, true);

    let cloned = Arc::clone(&counter);
    TrafficCounter::add(&cloned, 50, false);

    // 两个实例应该共享同一个计数器
    assert_eq!(counter.get_upload(), 100);
    assert_eq!(counter.get_download(), 50);
    assert_eq!(cloned.get_upload(), 100);
    assert_eq!(cloned.get_download(), 50);
}
