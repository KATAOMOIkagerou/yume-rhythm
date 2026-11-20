//! Pure-Rust audio synthesizers:
//! - 3-tone countdown completion chime (`[660, 880, 1046]` Hz @ 0 / 220 / 440 ms)
//! - 16-step drum & bass demo synthesizer (`A1` port, default 112 BPM)

use std::f32::consts::TAU;

/// Generates the exact 3-tone ascending completion chime matching `src/state/timer.ts`.
///
/// Notes: `[660, 880, 1046]` Hz starting at `0.00s`, `0.22s`, `0.44s`.
/// Per-note envelope: 20 ms linear attack to 0.18, 380 ms exponential decay to 0.001, stop at 420 ms.
pub fn generate_completion_chime(sample_rate: u32) -> Vec<f32> {
    let sr = sample_rate.max(8_000) as f32;
    let freqs = [660.0_f32, 880.0_f32, 1046.0_f32];
    let step_sec = 0.22_f32;
    let attack_sec = 0.02_f32;
    let decay_end_sec = 0.40_f32;
    let stop_sec = 0.42_f32;
    let peak_gain = 0.18_f32;
    let end_gain = 0.001_f32;

    let total_sec = step_sec * 2.0 + stop_sec + 0.04; // slight tail padding
    let total_samples = (total_sec * sr).ceil() as usize;
    let mut out = vec![0.0_f32; total_samples];

    for (idx, &freq) in freqs.iter().enumerate() {
        let start_sec = idx as f32 * step_sec;
        let start_sample = (start_sec * sr).round() as usize;
        let note_samples = (stop_sec * sr).ceil() as usize;

        for i in 0..note_samples {
            let pos = start_sample + i;
            if pos >= out.len() {
                break;
            }
            let t = i as f32 / sr;
            let env = if t < attack_sec {
                peak_gain * (t / attack_sec)
            } else if t < decay_end_sec {
                let u = (t - attack_sec) / (decay_end_sec - attack_sec);
                peak_gain * (end_gain / peak_gain).powf(u)
            } else if t < stop_sec {
                let u = (t - decay_end_sec) / (stop_sec - decay_end_sec);
                end_gain * (1.0 - u).max(0.0)
            } else {
                0.0
            };
            out[pos] += (TAU * freq * t).sin() * env;
        }
    }

    for sample in &mut out {
        *sample = sample.clamp(-0.95, 0.95);
    }
    out
}

struct KickVoice {
    age: f32,
    phase: f32,
    amp: f32,
}

struct SnareVoice {
    age: f32,
    bp_z1: f32,
    bp_z2: f32,
    bp_x1: f32,
    bp_x2: f32,
}

struct HihatVoice {
    age: f32,
    amp: f32,
    hp_y1: f32,
    hp_x1: f32,
}

struct BassVoice {
    age: f32,
    freq: f32,
    phase: f32,
    lp_y: f32,
}

/// Pure-Rust port of the `A1` 16-step drum & bass demo synthesizer.
pub struct DemoSynth {
    sample_rate: f32,
    bpm: f32,
    step: usize,
    samples_until_step: f32,
    rng_state: u64,
    kicks: Vec<KickVoice>,
    snares: Vec<SnareVoice>,
    hihats: Vec<HihatVoice>,
    basses: Vec<BassVoice>,
}

const BASS_PATTERN: [i32; 16] = [0, -1, -1, 0, -1, -1, 7, -1, 0, -1, -1, 0, -1, 5, -1, 3];

impl DemoSynth {
    pub fn new(sample_rate: u32, bpm: f32) -> Self {
        let clamped_bpm = if bpm.is_finite() {
            bpm.clamp(50.0, 220.0)
        } else {
            112.0
        };
        Self {
            sample_rate: sample_rate.max(8_000) as f32,
            bpm: clamped_bpm,
            step: 0,
            samples_until_step: 0.0,
            rng_state: 0x9E37_79B9_7F4A_7C15,
            kicks: Vec::with_capacity(4),
            snares: Vec::with_capacity(4),
            hihats: Vec::with_capacity(4),
            basses: Vec::with_capacity(4),
        }
    }

    #[inline]
    fn next_noise(&mut self) -> f32 {
        let mut x = self.rng_state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng_state = x;
        let unit = ((x >> 40) as f32) / ((1u32 << 24) as f32);
        unit * 2.0 - 1.0
    }

    fn trigger_step(&mut self, step: usize) {
        if matches!(step, 0 | 4 | 6 | 8 | 12 | 14) {
            let amp = if step == 6 || step == 14 { 0.75 } else { 1.0 };
            self.kicks.push(KickVoice {
                age: 0.0,
                phase: 0.0,
                amp,
            });
        }
        if step == 4 || step == 12 {
            self.snares.push(SnareVoice {
                age: 0.0,
                bp_z1: 0.0,
                bp_z2: 0.0,
                bp_x1: 0.0,
                bp_x2: 0.0,
            });
        }
        if step % 2 == 0 {
            let amp = if step % 4 == 0 { 0.35 } else { 0.6 };
            self.hihats.push(HihatVoice {
                age: 0.0,
                amp,
                hp_y1: 0.0,
                hp_x1: 0.0,
            });
        }
        let note = BASS_PATTERN[step % 16];
        if note >= 0 {
            let freq = 55.0 * 2.0_f32.powf(note as f32 / 12.0);
            self.basses.push(BassVoice {
                age: 0.0,
                freq,
                phase: 0.0,
                lp_y: 0.0,
            });
        }
    }

    /// Synthesizes mono `f32` samples into `out`.
    pub fn render_mono(&mut self, out: &mut [f32]) {
        let dt = 1.0 / self.sample_rate;
        let step_samples = (60.0 / self.bpm / 4.0) * self.sample_rate;

        // Precompute biquad bandpass coefficients for snare (1900 Hz, Q = 0.8)
        let w0 = TAU * 1900.0 / self.sample_rate;
        let alpha = w0.sin() / (2.0 * 0.8);
        let cos_w0 = w0.cos();
        let a0 = 1.0 + alpha;
        let b0 = alpha / a0;
        let b2 = -alpha / a0;
        let a1 = (-2.0 * cos_w0) / a0;
        let a2 = (1.0 - alpha) / a0;

        // 1-pole highpass coefficient for hi-hat (~7800 Hz)
        let rc_hp = 1.0 / (TAU * 7800.0);
        let hp_alpha = rc_hp / (rc_hp + dt);

        for sample in out.iter_mut() {
            if self.samples_until_step <= 0.0 {
                let s = self.step;
                self.trigger_step(s);
                self.step = (self.step + 1) % 16;
                self.samples_until_step += step_samples;
            }
            self.samples_until_step -= 1.0;

            let mut mix = 0.0_f32;

            // Kicks: sine sweep 160 Hz -> 46 Hz over 110 ms
            self.kicks.retain_mut(|k| {
                if k.age >= 0.30 {
                    return false;
                }
                let freq = if k.age < 0.11 {
                    160.0 * (46.0_f32 / 160.0).powf(k.age / 0.11)
                } else {
                    46.0
                };
                k.phase += TAU * freq * dt;
                let env = if k.age < 0.004 {
                    1e-4 * (k.amp / 1e-4).powf(k.age / 0.004)
                } else if k.age < 0.26 {
                    k.amp * (1e-4 / k.amp).powf((k.age - 0.004) / 0.256)
                } else {
                    0.0
                };
                mix += k.phase.sin() * env;
                k.age += dt;
                true
            });

            // Snares: bandpass-filtered noise
            let mut snare_idx = 0;
            while snare_idx < self.snares.len() {
                let age = self.snares[snare_idx].age;
                if age >= 0.20 {
                    self.snares.swap_remove(snare_idx);
                    continue;
                }
                let x = self.next_noise();
                let s = &mut self.snares[snare_idx];
                let y = b0 * x + b2 * s.bp_x2 - a1 * s.bp_z1 - a2 * s.bp_z2;
                s.bp_x2 = s.bp_x1;
                s.bp_x1 = x;
                s.bp_z2 = s.bp_z1;
                s.bp_z1 = y;
                let env = if age < 0.17 {
                    0.5 * (1e-4_f32 / 0.5).powf(age / 0.17)
                } else {
                    0.0
                };
                mix += y * env;
                s.age += dt;
                snare_idx += 1;
            }

            // Hi-hats: highpass-filtered noise
            let mut hh_idx = 0;
            while hh_idx < self.hihats.len() {
                let age = self.hihats[hh_idx].age;
                if age >= 0.08 {
                    self.hihats.swap_remove(hh_idx);
                    continue;
                }
                let x = self.next_noise();
                let h = &mut self.hihats[hh_idx];
                let y = hp_alpha * (h.hp_y1 + x - h.hp_x1);
                h.hp_x1 = x;
                h.hp_y1 = y;
                let peak = h.amp * 0.18;
                let env = if age < 0.05 {
                    peak * (1e-4_f32 / peak).powf(age / 0.05)
                } else {
                    0.0
                };
                mix += y * env;
                h.age += dt;
                hh_idx += 1;
            }

            // Sawtooth bass with decaying lowpass cutoff (900 -> 220 Hz)
            self.basses.retain_mut(|b| {
                if b.age >= 0.28 {
                    return false;
                }
                b.phase = (b.phase + b.freq * dt).fract();
                let saw = 2.0 * b.phase - 1.0;
                let cutoff = if b.age < 0.22 {
                    900.0 * (220.0_f32 / 900.0).powf(b.age / 0.22)
                } else {
                    220.0
                };
                let rc = 1.0 / (TAU * cutoff);
                let lp_a = dt / (rc + dt);
                b.lp_y += lp_a * (saw - b.lp_y);
                let env = if b.age < 0.01 {
                    1e-4 * (0.28_f32 / 1e-4).powf(b.age / 0.01)
                } else if b.age < 0.24 {
                    0.28 * (1e-4_f32 / 0.28).powf((b.age - 0.01) / 0.23)
                } else {
                    0.0
                };
                mix += b.lp_y * env;
                b.age += dt;
                true
            });

            *sample = (mix * 0.9).clamp(-1.0, 1.0);
        }
    }
}
