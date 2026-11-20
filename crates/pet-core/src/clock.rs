use std::sync::OnceLock;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

static MONOTONIC_EPOCH: OnceLock<Instant> = OnceLock::new();

/// Process-wide monotonic milliseconds clock shared by DSP beat prediction (`next_beat_at_ms`),
/// locked beat nod curve (`m1`), and character animation physics.
pub fn monotonic_ms() -> f64 {
    let epoch = MONOTONIC_EPOCH.get_or_init(Instant::now);
    epoch.elapsed().as_secs_f64() * 1000.0
}

/// Wall-clock Unix epoch milliseconds used for drift-free bidirectional study timer anchors.
pub fn wall_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Lightweight deterministic-capable xorshift64* PRNG to avoid pulling heavy RNG crates.
#[derive(Debug, Clone)]
pub struct FastRng {
    state: u64,
}

impl FastRng {
    pub fn from_entropy() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E37_79B9_7F4A_7C15);
        let mono = (monotonic_ms().to_bits()).rotate_left(17);
        let seed = nanos ^ mono ^ 0xA076_1D64_78BD_642F;
        Self::from_seed(if seed == 0 { 0xCAFE_BABE_DEAD_BEEF } else { seed })
    }

    pub fn from_seed(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 1 } else { seed },
        }
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Returns a uniform float in `[0.0, 1.0)`.
    pub fn next_f32(&mut self) -> f32 {
        ((self.next_u64() >> 40) as f32) / ((1u32 << 24) as f32)
    }

    /// Returns a uniform float in `[min, max)`.
    pub fn range_f32(&mut self, min: f32, max: f32) -> f32 {
        min + self.next_f32() * (max - min)
    }

    /// Returns a uniform index in `0..len`.
    pub fn next_usize(&mut self, len: usize) -> usize {
        if len == 0 {
            0
        } else {
            (self.next_u64() as usize) % len
        }
    }
}
