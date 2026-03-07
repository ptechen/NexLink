use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::collections::VecDeque;
use std::time::Instant;

/// ML-based prediction models for traffic forecasting
#[derive(Debug, Clone)]
pub struct TrafficPrediction {
    pub predicted_upload_bps: f64,
    pub predicted_download_bps: f64,
    pub predicted_connections: u32,
    pub confidence: f64,
    pub timestamp: Instant,
}

impl Default for TrafficPrediction {
    fn default() -> Self {
        Self {
            predicted_upload_bps: 0.0,
            predicted_download_bps: 0.0,
            predicted_connections: 0,
            confidence: 0.0,
            timestamp: Instant::now(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TrafficHistory {
    pub timestamps: VecDeque<Instant>,
    pub bytes_sent: VecDeque<u64>,
    pub bytes_received: VecDeque<u64>,
    pub connection_counts: VecDeque<u32>,
}

impl TrafficHistory {
    pub fn new() -> Self {
        Self {
            timestamps: VecDeque::new(),
            bytes_sent: VecDeque::new(),
            bytes_received: VecDeque::new(),
            connection_counts: VecDeque::new(),
        }
    }

    pub fn push_sample(&mut self, timestamp: Instant, sent: u64, received: u64, connections: u32) {
        // Keep only the last 100 samples to prevent memory growth
        if self.timestamps.len() >= 100 {
            self.timestamps.pop_front();
            self.bytes_sent.pop_front();
            self.bytes_received.pop_front();
            self.connection_counts.pop_front();
        }

        self.timestamps.push_back(timestamp);
        self.bytes_sent.push_back(sent);
        self.bytes_received.push_back(received);
        self.connection_counts.push_back(connections);
    }

    pub fn get_bandwidth_stats(&self) -> (f64, f64) { // (upload_bps, download_bps)
        if self.timestamps.len() < 2 {
            return (0.0, 0.0);
        }

        let time_diff = self.timestamps.back().unwrap().duration_since(*self.timestamps.front().unwrap()).as_secs_f64();
        if time_diff == 0.0 {
            return (0.0, 0.0);
        }

        let total_sent = self.bytes_sent.back().unwrap() - self.bytes_sent.front().unwrap();
        let total_received = self.bytes_received.back().unwrap() - self.bytes_received.front().unwrap();

        (total_sent as f64 / time_diff, total_received as f64 / time_diff)
    }

    /// Calculate moving average of bandwidth over the last N samples
    pub fn get_moving_average_bandwidth(&self, n: usize) -> (f64, f64) {
        if self.timestamps.len() < 2 || n < 2 {
            return self.get_bandwidth_stats();
        }

        let samples_to_consider = std::cmp::min(n, self.timestamps.len());

        // Get the range of indices to calculate average
        let start_idx = self.timestamps.len() - samples_to_consider;

        let mut total_time = 0.0;
        let mut total_sent = 0u64;
        let mut total_received = 0u64;

        for i in start_idx..(self.timestamps.len() - 1) {
            let time_diff = self.timestamps[i + 1].duration_since(self.timestamps[i]).as_secs_f64();
            let sent_delta = self.bytes_sent[i + 1] - self.bytes_sent[i];
            let received_delta = self.bytes_received[i + 1] - self.bytes_received[i];

            total_time += time_diff;
            total_sent += sent_delta;
            total_received += received_delta;
        }

        if total_time == 0.0 {
            return (0.0, 0.0);
        }

        (total_sent as f64 / total_time, total_received as f64 / total_time)
    }

    /// Predict future traffic based on historical patterns using linear regression
    pub fn predict_bandwidth(&self) -> TrafficPrediction {
        if self.timestamps.len() < 3 {
            return TrafficPrediction::default();
        }

        // Perform linear regression to predict trends
        let n = self.timestamps.len() as f64;
        let mut sum_x = 0.0;
        let mut sum_y_upload = 0.0;
        let mut sum_y_download = 0.0;
        let mut sum_x_sq = 0.0;
        let mut sum_xy_upload = 0.0;
        let mut sum_xy_download = 0.0;

        let start_time = self.timestamps.front().unwrap().elapsed().as_secs_f64();

        for (i, ((&timestamp, &sent), &received)) in self.timestamps.iter()
            .zip(self.bytes_sent.iter())
            .zip(self.bytes_received.iter())
            .enumerate()
        {
            let x = timestamp.elapsed().as_secs_f64() - start_time;
            let time_diff = if i > 0 {
                (timestamp.duration_since(self.timestamps[i - 1]).as_secs_f64()).max(0.1)
            } else {
                1.0
            };

            let upload_rate = if i > 0 {
                ((sent - self.bytes_sent[i - 1]) as f64 / time_diff).max(0.0)
            } else {
                0.0
            };

            let download_rate = if i > 0 {
                ((received - self.bytes_received[i - 1]) as f64 / time_diff).max(0.0)
            } else {
                0.0
            };

            sum_x += x;
            sum_y_upload += upload_rate;
            sum_y_download += download_rate;
            sum_x_sq += x * x;
            sum_xy_upload += x * upload_rate;
            sum_xy_download += x * download_rate;
        }

        // Calculate regression coefficients for upload
        let denominator = n * sum_x_sq - sum_x * sum_x;
        let (slope_upload, intercept_upload) = if denominator != 0.0 {
            let slope = (n * sum_xy_upload - sum_x * sum_y_upload) / denominator;
            let intercept = (sum_y_upload - slope * sum_x) / n;
            (slope, intercept)
        } else {
            (0.0, sum_y_upload / n)
        };

        // Calculate regression coefficients for download
        let (slope_download, intercept_download) = if denominator != 0.0 {
            let slope = (n * sum_xy_download - sum_x * sum_y_download) / denominator;
            let intercept = (sum_y_download - slope * sum_x) / n;
            (slope, intercept)
        } else {
            (0.0, sum_y_download / n)
        };

        // Predict next values (assuming 1-second ahead prediction)
        let next_x = self.timestamps.back().unwrap().elapsed().as_secs_f64() - start_time + 1.0;
        let predicted_upload = slope_upload * next_x + intercept_upload;
        let predicted_download = slope_download * next_x + intercept_download;

        // Calculate confidence based on data variance
        let mut variance_upload = 0.0;
        let mut variance_download = 0.0;
        for (i, &timestamp) in self.timestamps.iter().enumerate() {
            let x = timestamp.elapsed().as_secs_f64() - start_time;
            let actual_upload = if i > 0 {
                let time_diff = (timestamp.duration_since(self.timestamps[i - 1]).as_secs_f64()).max(0.1);
                ((self.bytes_sent[i] - self.bytes_sent[i - 1]) as f64 / time_diff).max(0.0)
            } else { 0.0 };

            let actual_download = if i > 0 {
                let time_diff = (timestamp.duration_since(self.timestamps[i - 1]).as_secs_f64()).max(0.1);
                ((self.bytes_received[i] - self.bytes_received[i - 1]) as f64 / time_diff).max(0.0)
            } else { 0.0 };

            let predicted_at_x = slope_upload * x + intercept_upload;
            variance_upload += (actual_upload - predicted_at_x).powi(2);

            let predicted_at_x = slope_download * x + intercept_download;
            variance_download += (actual_download - predicted_at_x).powi(2);
        }

        let avg_variance = (variance_upload + variance_download) / (2.0 * n.max(1.0));
        let confidence = (1.0 / (1.0 + avg_variance.sqrt())).min(1.0); // Normalize confidence to 0-1

        TrafficPrediction {
            predicted_upload_bps: predicted_upload.max(0.0),
            predicted_download_bps: predicted_download.max(0.0),
            predicted_connections: self.connection_counts.back().copied().unwrap_or(0),
            confidence,
            timestamp: Instant::now(),
        }
    }

    /// Detect anomalies in traffic patterns
    pub fn detect_anomalies(&self) -> Vec<(usize, String)> {
        let mut anomalies = Vec::new();

        if self.timestamps.len() < 3 {
            return anomalies;
        }

        // Calculate moving average and standard deviation
        let mut averages = Vec::new();
        let mut std_devs = Vec::new();

        for i in 2..self.timestamps.len() {
            // Calculate rolling average for the past 3 points
            let start = if i >= 3 { i - 3 } else { 0 };
            let mut sum = 0.0;
            for j in start..i {
                let time_diff = (self.timestamps[j].duration_since(self.timestamps[j.saturating_sub(1)]).as_secs_f64()).max(0.1);
                let bytes = if j > 0 { (self.bytes_sent[j] - self.bytes_sent[j-1]) as f64 / time_diff } else { 0.0 };
                sum += bytes;
            }
            let avg = sum / (i - start) as f64;

            // Calculate std dev
            let mut sum_sq_diff = 0.0;
            for j in start..i {
                let time_diff = (self.timestamps[j].duration_since(self.timestamps[j.saturating_sub(1)]).as_secs_f64()).max(0.1);
                let bytes = if j > 0 { (self.bytes_sent[j] - self.bytes_sent[j-1]) as f64 / time_diff } else { 0.0 };
                sum_sq_diff += (bytes - avg).powi(2);
            }
            let std_dev = (sum_sq_diff / (i - start) as f64).sqrt();

            averages.push(avg);
            std_devs.push(std_dev);
        }

        // Check for anomalies (values more than 2 std devs from the mean)
        for (i, (&avg, &std_dev)) in averages.iter().zip(std_devs.iter()).enumerate() {
            if i + 2 < self.timestamps.len() {
                let idx = i + 2;
                let time_diff = (self.timestamps[idx].duration_since(self.timestamps[idx.saturating_sub(1)]).as_secs_f64()).max(0.1);
                let bytes = if idx > 0 { (self.bytes_sent[idx] - self.bytes_sent[idx-1]) as f64 / time_diff } else { 0.0 };

                if (bytes - avg).abs() > 2.0 * std_dev {
                    anomalies.push((idx, format!("Upload anomaly: observed {} vs expected {}±{}",
                        bytes, avg, std_dev)));
                }
            }
        }

        anomalies
    }
}

#[derive(Debug, Clone)]
pub struct TrafficCounter {
    pub bytes_sent: Arc<AtomicU64>,
    pub bytes_received: Arc<AtomicU64>,
    pub active_connections: Arc<AtomicU32>,
    pub history: Arc<parking_lot::Mutex<TrafficHistory>>,
    pub predictions: Arc<parking_lot::Mutex<Vec<TrafficPrediction>>>,
}

impl TrafficCounter {
    pub fn new() -> Self {
        Self {
            bytes_sent: Arc::new(AtomicU64::new(0)),
            bytes_received: Arc::new(AtomicU64::new(0)),
            active_connections: Arc::new(AtomicU32::new(0)),
            history: Arc::new(parking_lot::Mutex::new(TrafficHistory::new())),
            predictions: Arc::new(parking_lot::Mutex::new(Vec::new())),
        }
    }

    pub fn add_sent(&self, n: u64) {
        self.bytes_sent.fetch_add(n, Ordering::Relaxed);
        self.update_history();
        self.update_predictions();
    }

    pub fn add_received(&self, n: u64) {
        self.bytes_received.fetch_add(n, Ordering::Relaxed);
        self.update_history();
        self.update_predictions();
    }

    pub fn inc_connections(&self) {
        self.active_connections.fetch_add(1, Ordering::Relaxed);
        self.update_history();
        self.update_predictions();
    }

    pub fn dec_connections(&self) {
        self.active_connections.fetch_sub(1, Ordering::Relaxed);
        self.update_history();
        self.update_predictions();
    }

    fn update_history(&self) {
        let current_sent = self.bytes_sent.load(Ordering::Relaxed);
        let current_received = self.bytes_received.load(Ordering::Relaxed);
        let current_connections = self.active_connections.load(Ordering::Relaxed);

        let mut history = self.history.lock();
        history.push_sample(Instant::now(), current_sent, current_received, current_connections);
    }

    fn update_predictions(&self) {
        let history = self.history.lock();
        let prediction = history.predict_bandwidth();

        let mut predictions = self.predictions.lock();
        // Keep only the last 10 predictions
        if predictions.len() >= 10 {
            predictions.remove(0);
        }
        predictions.push(prediction);
    }

    pub fn get_current_prediction(&self) -> Option<TrafficPrediction> {
        let predictions = self.predictions.lock();
        predictions.last().cloned()
    }

    pub fn get_anomaly_report(&self) -> Vec<(usize, String)> {
        let history = self.history.lock();
        history.detect_anomalies()
    }

    pub fn snapshot(&self) -> TrafficSnapshot {
        let history = self.history.lock();
        let (upload_bps, download_bps) = history.get_bandwidth_stats();

        let prediction = {
            let predictions = self.predictions.lock();
            predictions.last().cloned()
        };

        TrafficSnapshot {
            bytes_sent: self.bytes_sent.load(Ordering::Relaxed),
            bytes_received: self.bytes_received.load(Ordering::Relaxed),
            active_connections: self.active_connections.load(Ordering::Relaxed),
            bandwidth_upload: upload_bps,
            bandwidth_download: download_bps,
            predicted_upload: prediction.as_ref().map(|p| p.predicted_upload_bps),
            predicted_download: prediction.as_ref().map(|p| p.predicted_download_bps),
            prediction_confidence: prediction.as_ref().map(|p| p.confidence),
        }
    }
}

pub struct TrafficSnapshot {
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub active_connections: u32,
    pub bandwidth_upload: f64,
    pub bandwidth_download: f64,
    pub predicted_upload: Option<f64>,
    pub predicted_download: Option<f64>,
    pub prediction_confidence: Option<f64>,
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

/// Bidirectional relay with proper half-close semantics.
///
/// When one direction reaches EOF, we shutdown the write side of the other
/// direction and let the remaining direction drain completely. This prevents
/// in-flight data from being dropped (which `tokio::select!` would cause).
pub async fn relay_bidirectional<A, B>(
    a: &mut A,
    b: &mut B,
    a_to_b_counter: &AtomicU64,
    b_to_a_counter: &AtomicU64,
) where
    A: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    B: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    use tokio::io::AsyncWriteExt;

    let (mut a_read, mut a_write) = tokio::io::split(a);
    let (mut b_read, mut b_write) = tokio::io::split(b);

    // Run both directions concurrently. When the first finishes (reader EOF),
    // shutdown the opposite write half so the peer sees EOF, then let the
    // remaining direction drain to completion.
    let mut a_to_b_done = false;
    let mut b_to_a_done = false;

    // Phase 1: wait for the first direction to finish
    tokio::select! {
        r = counted_copy(&mut a_read, &mut b_write, a_to_b_counter) => {
            a_to_b_done = true;
            if let Err(e) = r { tracing::warn!("a->b: {e}"); }
        }
        r = counted_copy(&mut b_read, &mut a_write, b_to_a_counter) => {
            b_to_a_done = true;
            if let Err(e) = r { tracing::warn!("b->a: {e}"); }
        }
    }

    // Phase 2: shutdown the write half of the finished direction's peer,
    // then drain the remaining direction.
    if a_to_b_done {
        // a->b finished: shutdown b's write side so b->a's reader sees EOF
        let _ = b_write.shutdown().await;
        // Drain b->a
        if let Err(e) = counted_copy(&mut b_read, &mut a_write, b_to_a_counter).await {
            tracing::warn!("b->a drain: {e}");
        }
    } else if b_to_a_done {
        // b->a finished: shutdown a's write side so a->b's reader sees EOF
        let _ = a_write.shutdown().await;
        // Drain a->b
        if let Err(e) = counted_copy(&mut a_read, &mut b_write, a_to_b_counter).await {
            tracing::warn!("a->b drain: {e}");
        }
    }
}

/// Same as `relay_bidirectional` but without traffic counting.
pub async fn relay_bidirectional_uncounted<A, B>(
    a: &mut A,
    b: &mut B,
) where
    A: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    B: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let dummy_a = AtomicU64::new(0);
    let dummy_b = AtomicU64::new(0);
    relay_bidirectional(a, b, &dummy_a, &dummy_b).await;
}
