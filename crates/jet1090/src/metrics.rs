use rs1090::decode::DF;
use rs1090::prelude::*;
use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Runtime metrics for the jet1090 decode pipeline.
///
/// All counters use `AtomicU64` with relaxed ordering for minimal overhead.
pub struct Metrics {
    /// Unix timestamp (seconds) when jet1090 started
    start_time: u64,
    /// Raw messages received from all sources (before dedup)
    messages_received: AtomicU64,
    /// Messages emitted after deduplication
    messages_after_dedup: AtomicU64,
    /// Successful `Message::from_bytes` decodes
    decode_successes: AtomicU64,
    /// Failed `Message::from_bytes` decodes
    decode_errors: AtomicU64,
    /// BDS05/BDS06 position decode attempts
    position_attempts: AtomicU64,
    /// Position decodes that resolved lat/lon
    position_successes: AtomicU64,
    /// Decoded message counts by Downlink Format (indices 0–24)
    df_counts: [AtomicU64; 25],
}

impl std::fmt::Debug for Metrics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Metrics")
            .field("start_time", &self.start_time)
            .field(
                "messages_received",
                &self.messages_received.load(Ordering::Relaxed),
            )
            .finish()
    }
}

impl Metrics {
    pub fn new() -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("SystemTime before unix epoch")
            .as_secs();
        Self {
            start_time: now,
            messages_received: AtomicU64::new(0),
            messages_after_dedup: AtomicU64::new(0),
            decode_successes: AtomicU64::new(0),
            decode_errors: AtomicU64::new(0),
            position_attempts: AtomicU64::new(0),
            position_successes: AtomicU64::new(0),
            df_counts: core::array::from_fn(|_| AtomicU64::new(0)),
        }
    }

    pub fn record_received(&self) {
        self.messages_received.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_after_dedup(&self) {
        self.messages_after_dedup.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_decode_success(&self) {
        self.decode_successes.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_decode_error(&self) {
        self.decode_errors.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_position_attempt(&self) {
        self.position_attempts.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_position_success(&self) {
        self.position_successes.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_df(&self, df_num: u8) {
        let idx = (df_num as usize).min(24);
        self.df_counts[idx].fetch_add(1, Ordering::Relaxed);
    }

    /// Produce a serializable point-in-time snapshot of all counters.
    pub fn snapshot(&self, active_aircraft: usize) -> MetricsSnapshot {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("SystemTime before unix epoch")
            .as_secs();
        MetricsSnapshot {
            uptime_seconds: now - self.start_time,
            active_aircraft: active_aircraft as u64,
            messages_received: self.messages_received.load(Ordering::Relaxed),
            messages_after_dedup: self
                .messages_after_dedup
                .load(Ordering::Relaxed),
            decode_successes: self.decode_successes.load(Ordering::Relaxed),
            decode_errors: self.decode_errors.load(Ordering::Relaxed),
            position_attempts: self.position_attempts.load(Ordering::Relaxed),
            position_successes: self.position_successes.load(Ordering::Relaxed),
            df_counts: DfCounts {
                df0: self.df_counts[0].load(Ordering::Relaxed),
                df4: self.df_counts[4].load(Ordering::Relaxed),
                df5: self.df_counts[5].load(Ordering::Relaxed),
                df11: self.df_counts[11].load(Ordering::Relaxed),
                df16: self.df_counts[16].load(Ordering::Relaxed),
                df17: self.df_counts[17].load(Ordering::Relaxed),
                df18: self.df_counts[18].load(Ordering::Relaxed),
                df19: self.df_counts[19].load(Ordering::Relaxed),
                df20: self.df_counts[20].load(Ordering::Relaxed),
                df21: self.df_counts[21].load(Ordering::Relaxed),
                df24: self.df_counts[24].load(Ordering::Relaxed),
            },
        }
    }
}

/// Returns the numeric Downlink Format for a decoded message.
pub fn df_number(df: &DF) -> u8 {
    match df {
        ShortAirAirSurveillance { .. } => 0,
        SurveillanceAltitudeReply { .. } => 4,
        SurveillanceIdentityReply { .. } => 5,
        AllCallReply { .. } => 11,
        LongAirAirSurveillance { .. } => 16,
        ExtendedSquitterADSB(_) => 17,
        ExtendedSquitterTisB { .. } => 18,
        ExtendedSquitterMilitary { .. } => 19,
        CommBAltitudeReply { .. } => 20,
        CommBIdentityReply { .. } => 21,
        CommDExtended { .. } => 24,
    }
}

/// Serializable point-in-time view of all metrics.
#[derive(Serialize)]
pub struct MetricsSnapshot {
    pub uptime_seconds: u64,
    pub active_aircraft: u64,
    pub messages_received: u64,
    pub messages_after_dedup: u64,
    pub decode_successes: u64,
    pub decode_errors: u64,
    pub position_attempts: u64,
    pub position_successes: u64,
    pub df_counts: DfCounts,
}

/// Decoded message counts broken out by Downlink Format.
#[derive(Serialize)]
pub struct DfCounts {
    /// DF0 — Short Air-Air Surveillance
    pub df0: u64,
    /// DF4 — Surveillance Altitude Reply
    pub df4: u64,
    /// DF5 — Surveillance Identity Reply
    pub df5: u64,
    /// DF11 — All Call Reply
    pub df11: u64,
    /// DF16 — Long Air-Air Surveillance
    pub df16: u64,
    /// DF17 — ADS-B Extended Squitter
    pub df17: u64,
    /// DF18 — TIS-B Extended Squitter
    pub df18: u64,
    /// DF19 — Military Extended Squitter
    pub df19: u64,
    /// DF20 — Comm-B Altitude Reply
    pub df20: u64,
    /// DF21 — Comm-B Identity Reply
    pub df21: u64,
    /// DF24 — Comm-D Extended
    pub df24: u64,
}
