use crate::copy::CopyBuffer;
use std::sync::Arc;
use std::{
    future::poll_fn,
    io,
    pin::Pin,
    task::{Context, Poll, ready},
};
use tokio::io::{AsyncRead, AsyncWrite};

const DEFAULT_BUF_SIZE: usize = 32 * 1024;

/// 预设缓冲区大小常量
pub const SMALL_BUF_SIZE: usize = 8 * 1024; // 8KB - 适用于小文件/低内存场景
pub const LARGE_BUF_SIZE: usize = 64 * 1024; // 64KB - 适用于大文件/高吞吐场景

enum TransferState {
    Running(CopyBuffer),
    ShuttingDown,
    Done,
}
pub async fn copy_bidirectional<A, B, T>(a: &mut A, b: &mut B, info: &Arc<T>, exit_flag: &bool, times: &mut u64) -> io::Result<()>
where
    A: AsyncRead + AsyncWrite + Unpin + ?Sized,
    B: AsyncRead + AsyncWrite + Unpin + ?Sized,
    T: TrafficTrait,
{
    copy_bidirectional_impl(
        a,
        b,
        CopyBuffer::new(DEFAULT_BUF_SIZE),
        CopyBuffer::new(DEFAULT_BUF_SIZE),
        info,
        exit_flag,
        times,
    )
    .await
}

/// 使用自定义缓冲区大小的双向复制
///
/// # 参数
/// - `buffer_size`: 缓冲区大小（字节），建议使用预设常量 SMALL_BUF_SIZE/DEFAULT_BUF_SIZE/LARGE_BUF_SIZE
///
/// # 性能建议
/// - 小文件/低内存: 使用 SMALL_BUF_SIZE (8KB)
/// - 一般场景: 使用 DEFAULT_BUF_SIZE (32KB)
/// - 大文件/高吞吐: 使用 LARGE_BUF_SIZE (64KB)
pub async fn copy_bidirectional_with_buffer_size<A, B, T>(
    a: &mut A,
    b: &mut B,
    info: &Arc<T>,
    exit_flag: &bool,
    times: &mut u64,
    buffer_size: usize,
) -> io::Result<()>
where
    A: AsyncRead + AsyncWrite + Unpin + ?Sized,
    B: AsyncRead + AsyncWrite + Unpin + ?Sized,
    T: TrafficTrait,
{
    copy_bidirectional_impl(
        a,
        b,
        CopyBuffer::new(buffer_size),
        CopyBuffer::new(buffer_size),
        info,
        exit_flag,
        times,
    )
    .await
}

async fn copy_bidirectional_impl<A, B, T>(
    a: &mut A,
    b: &mut B,
    a_to_b_buffer: CopyBuffer,
    b_to_a_buffer: CopyBuffer,
    info: &Arc<T>,
    exit_flag: &bool,
    times: &mut u64,
) -> io::Result<()>
where
    A: AsyncRead + AsyncWrite + Unpin + ?Sized,
    B: AsyncRead + AsyncWrite + Unpin + ?Sized,
    T: TrafficTrait,
{
    let mut a_to_b = TransferState::Running(a_to_b_buffer);
    let mut b_to_a = TransferState::Running(b_to_a_buffer);
    poll_fn(|cx| {
        let a_to_b = transfer_one_direction(cx, &mut a_to_b, a, b, info, true, exit_flag, times)?;
        let b_to_a = transfer_one_direction(cx, &mut b_to_a, b, a, info, false, exit_flag, times)?;

        // It is not a problem if ready! returns early because transfer_one_direction for the
        // other direction will keep returning TransferState::Done(count) in future calls to poll
        ready!(a_to_b);
        ready!(b_to_a);

        Poll::Ready(Ok(()))
    })
    .await
}

#[allow(clippy::too_many_arguments)]
fn transfer_one_direction<A, B, T>(
    cx: &mut Context<'_>,
    state: &mut TransferState,
    r: &mut A,
    w: &mut B,
    info: &Arc<T>,
    is_upload: bool,
    exit_flag: &bool,
    times: &mut u64,
) -> Poll<io::Result<()>>
where
    A: AsyncRead + AsyncWrite + Unpin + ?Sized,
    B: AsyncRead + AsyncWrite + Unpin + ?Sized,
    T: TrafficTrait,
{
    let mut r = Pin::new(r);
    let mut w = Pin::new(w);
    loop {
        match state {
            TransferState::Running(buf) => {
                ready!(buf.poll_copy(cx, r.as_mut(), w.as_mut(), info, is_upload, exit_flag, times,))?;
                *state = TransferState::ShuttingDown;
            }
            TransferState::ShuttingDown => {
                ready!(w.as_mut().poll_shutdown(cx))?;
                *state = TransferState::Done;
            }
            TransferState::Done => return Poll::Ready(Ok(())),
        }
    }
}

pub trait TrafficTrait: Send + Sync {
    fn add(info: &Arc<Self>, size: u64, is_upload: bool);
}
