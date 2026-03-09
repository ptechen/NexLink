use crate::copy_bidirectional::TrafficTrait;
use std::sync::Arc;
use std::{
    io,
    pin::Pin,
    task::{Context, Poll, ready},
};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

#[derive(Debug)]
pub struct CopyBuffer {
    read_done: bool,
    pos: usize,
    cap: usize,
    amt: usize,
    buf: Box<[u8]>,
}

const TRAFFIC_STATISTICS_SIZE: usize = 1024 * 256;
impl CopyBuffer {
    pub fn new(buf_size: usize) -> Self {
        Self {
            read_done: false,
            pos: 0,
            cap: 0,
            amt: 0,
            buf: {
                // SAFETY: 缓冲区在使用前会被 poll_fill_buf 填充
                // - ReadBuf::new 创建时会重置填充指针
                // - poll_write_buf 只访问 [pos..cap] 已填充范围
                // - 未初始化区域永远不会被读取
                let mut vec = Vec::with_capacity(buf_size);
                unsafe {
                    vec.set_len(buf_size);
                }
                vec.into_boxed_slice()
            },
        }
    }

    fn poll_fill_buf<R>(&mut self, cx: &mut Context<'_>, reader: Pin<&mut R>) -> Poll<io::Result<()>>
    where
        R: AsyncRead + ?Sized,
    {
        let me = &mut *self;
        let mut buf = ReadBuf::new(&mut me.buf);
        buf.set_filled(me.cap);

        let res = reader.poll_read(cx, &mut buf);
        if let Poll::Ready(Ok(())) = res {
            let filled_len = buf.filled().len();
            me.read_done = me.cap == filled_len;
            me.cap = filled_len;
        }
        res
    }

    fn poll_write_buf<R, W>(&mut self, cx: &mut Context<'_>, mut reader: Pin<&mut R>, mut writer: Pin<&mut W>) -> Poll<io::Result<usize>>
    where
        R: AsyncRead + ?Sized,
        W: AsyncWrite + ?Sized,
    {
        let me = &mut *self;
        match writer.as_mut().poll_write(cx, &me.buf[me.pos..me.cap]) {
            Poll::Pending => {
                // Top up the buffer towards full if we can read a bit more
                // data - this should improve the chances of a large write
                if !me.read_done && me.cap < me.buf.len() {
                    ready!(me.poll_fill_buf(cx, reader.as_mut()))?;
                }
                Poll::Pending
            }
            res => res,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn poll_copy<R, W, T>(
        &mut self,
        cx: &mut Context<'_>,
        mut reader: Pin<&mut R>,
        mut writer: Pin<&mut W>,
        traffic_fn: &Arc<T>,
        is_upload: bool,
        exit_flag: &bool,
        times: &mut u64,
    ) -> Poll<io::Result<()>>
    where
        R: AsyncRead + ?Sized,
        W: AsyncWrite + ?Sized,
        T: TrafficTrait,
    {
        let coop = ready!(tokio::task::coop::poll_proceed(cx));

        loop {
            // If there is some space left in our buffer, then we try to read some
            // data to continue, thus maximizing the chances of a large write.
            if self.cap < self.buf.len() && !self.read_done {
                match self.poll_fill_buf(cx, reader.as_mut()) {
                    Poll::Ready(Ok(())) => {
                        coop.made_progress();
                    }
                    Poll::Ready(Err(err)) => {
                        if self.amt > 0 {
                            TrafficTrait::add(traffic_fn, self.amt as u64, is_upload);
                            self.amt = 0;
                        }
                        coop.made_progress();
                        return Poll::Ready(Err(err));
                    }
                    Poll::Pending => {
                        // Ignore pending reads when our buffer is not empty, because
                        // we can try to write data immediately.
                        if self.pos == self.cap || *exit_flag {
                            ready!(writer.as_mut().poll_flush(cx))?;
                            coop.made_progress();
                            if self.amt > 0 {
                                TrafficTrait::add(traffic_fn, self.amt as u64, is_upload);
                                self.amt = 0;
                            }
                            if *exit_flag {
                                return Poll::Ready(Err(io::Error::new(io::ErrorKind::TimedOut, "shutdown")));
                            }
                            return Poll::Pending;
                        }
                    }
                }
            }

            // 如果我们的缓冲区有一些数据，让我们写出来！
            while self.pos < self.cap {
                let i = ready!(self.poll_write_buf(cx, reader.as_mut(), writer.as_mut()))?;
                if i == 0 {
                    coop.made_progress();
                    if self.amt > 0 {
                        TrafficTrait::add(traffic_fn, self.amt as u64, is_upload);
                        self.amt = 0;
                    }
                    return Poll::Ready(Err(io::Error::new(io::ErrorKind::WriteZero, "write zero byte into writer")));
                } else {
                    self.pos += i;
                    self.amt += i;
                    *times += 1;
                    if self.amt > TRAFFIC_STATISTICS_SIZE {
                        TrafficTrait::add(traffic_fn, self.amt as u64, is_upload);
                        self.amt = 0;
                    }
                }
            }

            // If pos larger than cap, this loop will never stop.
            // In particular, user's wrong poll_write implementation returning
            // incorrect written length may lead to thread blocking.
            debug_assert!(self.pos <= self.cap, "writer returned length larger than input slice");

            // All data has been written, the buffer can be considered empty again
            self.pos = 0;
            self.cap = 0;

            // If we've written all the data and we've seen EOF, flush out the
            // data and finish the transfer.
            if self.read_done {
                if self.amt > 0 {
                    TrafficTrait::add(traffic_fn, self.amt as u64, is_upload);
                    self.amt = 0;
                }
                ready!(writer.as_mut().poll_flush(cx))?;
                coop.made_progress();
                return Poll::Ready(Ok(()));
            }
            self.read_done = *exit_flag;
            // info!("Poll::Pending");
            // if !self.read_done {
            //     self.read_done =
            //         CURRENT_TIME.load(Ordering::Relaxed) - self.start > TIMEOUT || *exit_flag;
            // }
        }
    }
}
