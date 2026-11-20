//! 合成测试信号：`tools/beat-baseline/lib/signals.mjs` 与 `lib/dsp.mjs` 的逐行转写。
//!
//! 只用 IEEE 基本运算（不调 sin/cos/exp），所以与 Node 生成的 Float32 采样逐位相同。
//! 两边必须同步修改。

use super::fixture::{Section, SynthSpec};

// 与 signals.mjs 的字面量 0.6931471805599453、6.283185307179586 是同一个 f64。
const LN2: f64 = std::f64::consts::LN_2;

/// JS `Math.round`。
fn js_round(x: f64) -> f64 {
    let c = x.ceil();
    if c - 0.5 > x { c - 1.0 } else { c }
}

/// 原 `dexp`：减半 + 泰勒 + 平方。
pub fn dexp(mut x: f64) -> f64 {
    let mut k = 0;
    while x > 0.5 || x < -0.5 {
        x *= 0.5;
        k += 1;
    }
    let mut term = 1.0;
    let mut sum = 1.0;
    for i in 1..=14 {
        term = (term * x) / f64::from(i);
        sum += term;
    }
    for _ in 0..k {
        sum *= sum;
    }
    sum
}

/// 原 `sinCycles`：`sin(2π·p)`，`p ∈ [0, 1)`。
pub fn sin_cycles(mut p: f64) -> f64 {
    let mut sign = 1.0;
    if p >= 0.5 {
        p -= 0.5;
        sign = -1.0;
    }
    if p > 0.25 {
        p = 0.5 - p;
    }
    let x = p * std::f64::consts::TAU;
    let x2 = x * x;
    let mut term = x;
    let mut sum = x;
    for i in 1..=8_i32 {
        term = (-term * x2) / f64::from(2 * i * (2 * i + 1));
        sum += term;
    }
    sign * sum
}

pub struct XorShift32(u32);

impl XorShift32 {
    pub fn new(seed: u32) -> Self {
        Self(if seed == 0 { 0x9e37_79b9 } else { seed })
    }

    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }

    pub fn next_signed(&mut self) -> f64 {
        (f64::from(self.next_u32()) / 4294967296.0) * 2.0 - 1.0
    }
}

pub fn synthesize(spec: &SynthSpec) -> Vec<f32> {
    let sr = spec.sample_rate;
    let total = js_round(spec.duration * sr) as usize;
    let mut acc = vec![0.0_f64; total];
    let mut rng = XorShift32::new(spec.seed);
    for sec in &spec.sections {
        render_section(&mut acc, sec, sr, &mut rng);
    }
    acc.iter()
        .map(|&a| (a * spec.gain).clamp(-1.0, 1.0) as f32)
        .collect()
}

fn render_section(acc: &mut [f64], sec: &Section, sr: f64, rng: &mut XorShift32) {
    let step_sec = 60.0 / sec.bpm / 4.0;
    let mut i = 0_usize;
    loop {
        let t = sec.start + i as f64 * step_sec;
        if t >= sec.end {
            break;
        }
        let at = (t * sr).floor() as usize;
        let s = i % 16;
        if sec.kick[s] > 0.0 {
            kick(acc, at, sec.kick[s], sr);
        }
        if sec.snare[s] > 0.0 {
            snare(acc, at, sec.snare[s], sr, rng);
        }
        if sec.hat[s] > 0.0 {
            hat(acc, at, sec.hat[s], sr, rng);
        }
        if let Some(semi) = sec.bass[s] {
            bass(acc, at, semi, sr);
        }
        i += 1;
    }
}

fn kick(acc: &mut [f64], at: usize, vel: f64, sr: f64) {
    let len = (0.32 * sr).floor() as usize;
    let attack = 0.004 * sr;
    let mut phase = 0.0_f64;
    let mut n = 0;
    while n < len && at + n < acc.len() {
        let nf = n as f64;
        let t = nf / sr;
        let env = (if nf < attack { nf / attack } else { 1.0 }) * dexp(-t / 0.13);
        acc[at + n] += vel * 0.9 * env * sin_cycles(phase);
        let f = 46.0 + 114.0 * dexp(-t / 0.035);
        phase += f / sr;
        phase -= phase.floor();
        n += 1;
    }
}

fn snare(acc: &mut [f64], at: usize, vel: f64, sr: f64, rng: &mut XorShift32) {
    let len = (0.2 * sr).floor() as usize;
    let mut phase = 0.0_f64;
    let mut n = 0;
    while n < len && at + n < acc.len() {
        let t = n as f64 / sr;
        let noise = rng.next_signed();
        let v = 0.45 * noise * dexp(-t / 0.045) + 0.3 * sin_cycles(phase) * dexp(-t / 0.03);
        acc[at + n] += vel * v;
        phase += 185.0 / sr;
        phase -= phase.floor();
        n += 1;
    }
}

fn hat(acc: &mut [f64], at: usize, vel: f64, sr: f64, rng: &mut XorShift32) {
    let len = (0.05 * sr).floor() as usize;
    let mut prev = 0.0;
    let mut n = 0;
    while n < len && at + n < acc.len() {
        let t = n as f64 / sr;
        let noise = rng.next_signed();
        acc[at + n] += vel * 0.12 * (noise - prev) * dexp(-t / 0.012);
        prev = noise;
        n += 1;
    }
}

fn bass(acc: &mut [f64], at: usize, semi: f64, sr: f64) {
    let len = (0.24 * sr).floor() as usize;
    let freq = 55.0 * dexp((semi / 12.0) * LN2);
    let attack = 0.01 * sr;
    let mut phase = 0.0_f64;
    let mut y = 0.0;
    let mut n = 0;
    while n < len && at + n < acc.len() {
        let nf = n as f64;
        let t = nf / sr;
        let saw = 2.0 * phase - 1.0;
        y += 0.06 * (saw - y);
        let env = (if nf < attack { nf / attack } else { 1.0 }) * dexp(-t / 0.09);
        acc[at + n] += 0.35 * y * env;
        phase += freq / sr;
        phase -= phase.floor();
        n += 1;
    }
}
