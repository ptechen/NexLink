use copy_bidirectional::copy_bidirectional::{
    LARGE_BUF_SIZE, SMALL_BUF_SIZE, TrafficTrait, copy_bidirectional,
    copy_bidirectional_with_buffer_size,
};
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::io;
use std::sync::Arc;

// 空流量统计实现（用于基准测试）
struct NoOpTraffic;

impl TrafficTrait for NoOpTraffic {
    fn add(_info: &Arc<Self>, _size: u64, _is_upload: bool) {}
}

// 创建内存缓冲区对（用于测试）
fn create_test_buffers(size: usize) -> (io::Cursor<Vec<u8>>, io::Cursor<Vec<u8>>) {
    let source_data = vec![0x42u8; size];
    let source = io::Cursor::new(source_data);
    let dest = io::Cursor::new(Vec::with_capacity(size));
    (source, dest)
}

fn bench_copy_data_sizes(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("copy_bidirectional");

    // 测试不同数据大小
    for size in [
        32 * 1024,  // 32KB - 中等数据
        64 * 1024,  // 32KB - 中等数据
        128 * 1024, // 32KB - 中等数据
        256 * 1024, // 32KB - 中等数据
        512 * 1024, // 32KB - 中等数据
    ] {
        group.throughput(Throughput::Bytes(size as u64));

        // 测试默认缓冲区大小
        group.bench_with_input(
            BenchmarkId::new("default", format!("{}KB", size / 1024)),
            &size,
            |b, &size| {
                b.to_async(&runtime).iter(|| async move {
                    let (mut source, mut dest) = create_test_buffers(size);
                    let mut times = 0u64;
                    let exit_flag = false;
                    let traffic = Arc::new(NoOpTraffic);

                    copy_bidirectional(&mut source, &mut dest, &traffic, &exit_flag, &mut times)
                        .await
                        .unwrap();
                });
            },
        );

        // 对于大数据，测试大缓冲区
        if size >= 1024 * 1024 {
            group.bench_with_input(
                BenchmarkId::new("large_buf", format!("{}MB", size / 1024 / 1024)),
                &size,
                |b, &size| {
                    b.to_async(&runtime).iter(|| async move {
                        let (mut source, mut dest) = create_test_buffers(size);
                        let mut times = 0u64;
                        let exit_flag = false;
                        let traffic = Arc::new(NoOpTraffic);

                        copy_bidirectional_with_buffer_size(
                            &mut source,
                            &mut dest,
                            &traffic,
                            &exit_flag,
                            &mut times,
                            LARGE_BUF_SIZE,
                        )
                        .await
                        .unwrap();
                    });
                },
            );
        }

        // 对于小数据，测试小缓冲区
        if size <= 32 * 1024 {
            group.bench_with_input(
                BenchmarkId::new("small_buf", format!("{}KB", size / 1024)),
                &size,
                |b, &size| {
                    b.to_async(&runtime).iter(|| async move {
                        let (mut source, mut dest) = create_test_buffers(size);
                        let mut times = 0u64;
                        let exit_flag = false;
                        let traffic = Arc::new(NoOpTraffic);

                        copy_bidirectional_with_buffer_size(
                            &mut source,
                            &mut dest,
                            &traffic,
                            &exit_flag,
                            &mut times,
                            SMALL_BUF_SIZE,
                        )
                        .await
                        .unwrap();
                    });
                },
            );
        }
    }

    group.finish();
}

criterion_group!(benches, bench_copy_data_sizes);
criterion_main!(benches);
