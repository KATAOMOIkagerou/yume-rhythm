use crate::audio_contract::AudioTelemetry;
use crate::clock::FastRng;
use std::f32::consts::PI;

const NOTE_GLYPHS: [char; 5] = ['♪', '♫', '♩', '♬', '✦'];
const BLINK_MIN_DELAY_MS: f64 = 2400.0;
const BLINK_MAX_DELAY_MS: f64 = 5200.0;
const BLINK_DURATION_MS: f64 = 150.0;
const NOTE_LIFETIME_MS: f64 = 1500.0;
const MAX_ACTIVE_NOTES: usize = 12;

/// Exact port of frozen `m1(now, nextBeatAt, periodMs, strength)` locked beat nod curve.
pub fn locked_beat_nod_m1(
    now_ms: f64,
    next_beat_at_ms: Option<f64>,
    period_ms: Option<f64>,
    strength: f32,
) -> Option<f32> {
    let (h, m) = (next_beat_at_ms?, period_ms?);
    if !h.is_finite() || !m.is_finite() || !(250.0..=1600.0).contains(&m) {
        return None;
    }
    let half = m * 0.5;
    let b = ((now_ms - h + half).rem_euclid(m)) - half;
    let x = 170.0_f64.min(m * 0.3);
    if b.abs() >= x {
        return Some(0.0);
    }
    let raised = (1.0 + (std::f64::consts::PI * b / x).cos()) * 0.5;
    let clamped_strength = (strength as f64).clamp(0.6, 3.0);
    Some((raised * clamped_strength * 3.4) as f32)
}

#[derive(Debug, Clone)]
pub struct NoteParticle {
    pub id: u64,
    pub left_pct: f32,
    pub top_pct: f32,
    pub glyph: char,
    pub hue: f32,
    pub drift_x: f32,
    pub born_ms: f64,
}

#[derive(Debug, Clone)]
pub struct NoteParticleRender {
    pub glyph: char,
    pub x_pct: f32,
    pub y_pct: f32,
    pub offset_x: f32,
    pub offset_y: f32,
    pub opacity: f32,
    pub scale: f32,
    pub rot_deg: f32,
    pub rgb: [u8; 3],
}

#[derive(Debug, Clone)]
pub struct AnimationPose {
    pub head_y: f32,
    pub head_rot_deg: f32,
    pub head_scale_x: f32,
    pub head_scale_y: f32,
    pub body_x: f32,
    pub body_y: f32,
    pub body_scale_x: f32,
    pub body_scale_y: f32,
    pub ring_radius: f32,
    pub ring_opacity: f32,
    pub ring_stroke_width: f32,
    pub fx_float_y: f32,
    pub fx_float_scale: f32,
    pub timer_glow_strength: f32,
    pub is_blinking: bool,
    pub notes: Vec<NoteParticleRender>,
}

impl Default for AnimationPose {
    fn default() -> Self {
        Self {
            head_y: 0.0,
            head_rot_deg: 0.0,
            head_scale_x: 1.0,
            head_scale_y: 1.0,
            body_x: 0.0,
            body_y: 0.0,
            body_scale_x: 1.0,
            body_scale_y: 1.0,
            ring_radius: 140.0,
            ring_opacity: 0.0,
            ring_stroke_width: 12.0,
            fx_float_y: 0.0,
            fx_float_scale: 1.0,
            timer_glow_strength: 0.0,
            is_blinking: false,
            notes: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BlinkController {
    blinking: bool,
    next_change_ms: f64,
    initialized: bool,
}

impl Default for BlinkController {
    fn default() -> Self {
        Self {
            blinking: false,
            next_change_ms: 0.0,
            initialized: false,
        }
    }
}

impl BlinkController {
    pub fn step(&mut self, now_ms: f64, suppress_blink: bool, rng: &mut FastRng) -> bool {
        if !self.initialized {
            self.initialized = true;
            self.blinking = false;
            self.next_change_ms =
                now_ms + rng.range_f32(BLINK_MIN_DELAY_MS as f32, BLINK_MAX_DELAY_MS as f32) as f64;
        }
        if suppress_blink {
            self.blinking = false;
            if now_ms >= self.next_change_ms {
                self.next_change_ms = now_ms
                    + rng.range_f32(BLINK_MIN_DELAY_MS as f32, BLINK_MAX_DELAY_MS as f32) as f64;
            }
            return false;
        }
        if now_ms >= self.next_change_ms {
            if self.blinking {
                self.blinking = false;
                self.next_change_ms = now_ms
                    + rng.range_f32(BLINK_MIN_DELAY_MS as f32, BLINK_MAX_DELAY_MS as f32) as f64;
            } else {
                self.blinking = true;
                self.next_change_ms = now_ms + BLINK_DURATION_MS;
            }
        }
        self.blinking
    }
}

#[derive(Debug, Clone)]
pub struct CharacterAnimator {
    head_y: f32,
    head_vy: f32,
    head_rot: f32,
    head_vrot: f32,
    scale_pulse: f32,
    scale_v: f32,
    squash_y: f32,
    doze_blend: f32,
    hit_strength: f32,
    ring_progress: f32,
    prev_beats: u64,
    last_time_ms: Option<f64>,
    next_note_id: u64,
    notes: Vec<NoteParticle>,
    blink: BlinkController,
    rng: FastRng,
}

impl Default for CharacterAnimator {
    fn default() -> Self {
        Self::new(FastRng::from_entropy())
    }
}

impl CharacterAnimator {
    pub fn new(rng: FastRng) -> Self {
        Self {
            head_y: 0.0,
            head_vy: 0.0,
            head_rot: 0.0,
            head_vrot: 0.0,
            scale_pulse: 1.0,
            scale_v: 0.0,
            squash_y: 1.0,
            doze_blend: 0.0,
            hit_strength: 1.8,
            ring_progress: 1.0,
            prev_beats: 0,
            last_time_ms: None,
            next_note_id: 0,
            notes: Vec::with_capacity(MAX_ACTIVE_NOTES),
            blink: BlinkController::default(),
            rng,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn step(
        &mut self,
        now_ms: f64,
        telemetry: &AudioTelemetry,
        music_running: bool,
        auto_follow: bool,
        is_dozing: bool,
        suppress_blink: bool,
        nod_amplitude_scale: f32,
        nod_tilt_scale: f32,
    ) -> AnimationPose {
        let dt_ms = match self.last_time_ms {
            Some(prev) => (now_ms - prev).clamp(0.0, 48.0) as f32,
            None => 16.667,
        };
        self.last_time_ms = Some(now_ms);
        let dt_norm = dt_ms / 16.667;

        let idle = !(music_running && auto_follow);
        let beat_triggered = !idle && telemetry.beats > self.prev_beats;
        self.prev_beats = telemetry.beats;

        if beat_triggered {
            let strength = 0.85 + telemetry.strength.min(2.0) * 1.15;
            self.hit_strength = strength;
            self.head_vy += strength * 1.2;
            self.head_vrot += (self.rng.next_f32() - 0.4) * strength * 0.6;
            self.scale_v += strength * 0.09;
            self.ring_progress = 0.0;
            if self.rng.next_f32() < 0.85 {
                self.spawn_note(now_ms);
            }
        }

        let level_val = if idle && telemetry.level <= 0.03 {
            0.1
        } else {
            telemetry.level
        };

        // Exact v1 vertical spring + m1 locked beat nod
        let damp_y = 0.86_f32.powf(dt_norm);
        self.head_vy += -0.115 * self.head_y * dt_norm;
        self.head_vy *= damp_y;
        self.head_y += self.head_vy * dt_norm;

        if !idle && telemetry.locked {
            if let Some(locked_y) = locked_beat_nod_m1(
                now_ms,
                telemetry.next_beat_at_ms,
                telemetry.period_ms,
                self.hit_strength,
            ) {
                self.head_y = locked_y;
                self.head_vy = 0.0;
            }
        }

        // Exact v1 tilt & bounce springs
        self.head_vrot += -0.09 * self.head_rot * dt_norm;
        self.head_vrot *= 0.9_f32.powf(dt_norm);
        self.head_rot += self.head_vrot * dt_norm;

        self.scale_v += -0.22 * (self.scale_pulse - 1.0) * dt_norm;
        self.scale_v *= 0.84_f32.powf(dt_norm);
        self.scale_pulse = (self.scale_pulse + self.scale_v * dt_norm).clamp(0.6, 1.6);

        let target_squash = 1.0 - (self.head_y * 0.004).clamp(-0.1, 0.1);
        self.squash_y += (target_squash - self.squash_y) * (0.25 * dt_norm).min(1.0);

        // Gentle idle breathing / music sway
        let breath_y = (now_ms / 340.0).sin() as f32 * (1.1 + level_val * 2.6);
        let sway_x = (now_ms / 520.0).sin() as f32 * (1.3 + level_val * 2.4);

        // Smooth non-music doze droop
        let target_doze = if idle && is_dozing { 1.0 } else { 0.0 };
        self.doze_blend += (target_doze - self.doze_blend) * (0.06 * dt_norm).min(1.0);
        let doze_y = self.doze_blend * (9.5 + (now_ms / 650.0).sin() as f32 * 3.5);
        let doze_rot = self.doze_blend * (-3.2 + (now_ms / 900.0).sin() as f32 * 1.6);

        let nod_visual_scale = 1.45_f32;
        let final_head_y = (self.head_y * nod_visual_scale * nod_amplitude_scale) + breath_y + doze_y;
        let final_head_rot = self.head_rot * nod_tilt_scale + doze_rot;
        let pulse_boost = (self.scale_pulse - 1.0).max(0.0) * 0.03;
        let head_scale_x = 2.0 - self.squash_y + pulse_boost;
        let head_scale_y = self.squash_y + pulse_boost;

        let body_x = sway_x * 0.65;
        let body_y = -self.head_y * 0.22 * nod_visual_scale * nod_amplitude_scale + breath_y * 0.4;

        let (ring_radius, ring_opacity, ring_stroke_width) = if self.ring_progress < 1.0 {
            self.ring_progress = (self.ring_progress + dt_ms / 620.0).min(1.0);
            let eased = 1.0 - (1.0 - self.ring_progress).powi(3);
            (
                140.0 + eased * 185.0,
                (1.0 - eased) * 0.42,
                12.0 * (1.0 - eased) + 1.5,
            )
        } else {
            (140.0, 0.0, 12.0)
        };

        // FX float (2.2s ease-in-out cycle) & timer glow (2.4s cycle)
        let fx_phase = ((now_ms % 2200.0) / 2200.0) as f32;
        let fx_wave = 0.5 - 0.5 * (fx_phase * 2.0 * PI).cos();
        let fx_float_y = -6.0 * fx_wave;
        let fx_float_scale = 1.0 + 0.03 * fx_wave;

        let timer_phase = ((now_ms % 2400.0) / 2400.0) as f32;
        let timer_glow_strength = 0.5 - 0.5 * (timer_phase * 2.0 * PI).cos();

        let is_blinking = self.blink.step(now_ms, suppress_blink, &mut self.rng);

        // Expire & interpolate musical note particles
        self.notes.retain(|n| now_ms - n.born_ms < NOTE_LIFETIME_MS);
        let notes = self
            .notes
            .iter()
            .map(|n| render_note(n, ((now_ms - n.born_ms) / NOTE_LIFETIME_MS) as f32))
            .collect();

        AnimationPose {
            head_y: final_head_y,
            head_rot_deg: final_head_rot,
            head_scale_x,
            head_scale_y,
            body_x,
            body_y,
            body_scale_x: self.squash_y,
            body_scale_y: 2.0 - self.squash_y,
            ring_radius,
            ring_opacity,
            ring_stroke_width,
            fx_float_y,
            fx_float_scale,
            timer_glow_strength,
            is_blinking,
            notes,
        }
    }

    fn spawn_note(&mut self, now_ms: f64) {
        self.next_note_id += 1;
        let idx = self.rng.next_usize(NOTE_GLYPHS.len());
        let particle = NoteParticle {
            id: self.next_note_id,
            left_pct: 30.0 + self.rng.next_f32() * 40.0,
            top_pct: 16.0 + self.rng.next_f32() * 10.0,
            glyph: NOTE_GLYPHS[idx],
            hue: 190.0 + self.rng.next_f32() * 130.0,
            drift_x: (self.rng.next_f32() - 0.5) * 34.0,
            born_ms: now_ms,
        };
        if self.notes.len() >= MAX_ACTIVE_NOTES {
            self.notes.remove(0);
        }
        self.notes.push(particle);
    }
}

fn render_note(n: &NoteParticle, t: f32) -> NoteParticleRender {
    let t = t.clamp(0.0, 1.0);
    let (opacity, offset_x, offset_y, scale, rot_deg) = if t < 0.18 {
        let u = t / 0.18;
        (
            u,
            n.drift_x * 0.25 * u,
            8.0 + (-14.0 - 8.0) * u,
            0.65 + (1.08 - 0.65) * u,
            -8.0 + (6.0 - -8.0) * u,
        )
    } else {
        let u = (t - 0.18) / 0.82;
        let ease = 1.0 - (1.0 - u).powi(3);
        (
            1.0 - ease,
            n.drift_x * (0.25 + 0.75 * ease),
            -14.0 + (-74.0 - -14.0) * ease,
            1.08 + (0.92 - 1.08) * ease,
            6.0 + (-10.0 - 6.0) * ease,
        )
    };
    NoteParticleRender {
        glyph: n.glyph,
        x_pct: n.left_pct,
        y_pct: n.top_pct,
        offset_x,
        offset_y,
        opacity,
        scale,
        rot_deg,
        rgb: hsl_to_rgb(n.hue, 0.88, 0.68),
    }
}

fn hsl_to_rgb(h_deg: f32, s: f32, l: f32) -> [u8; 3] {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let h = (h_deg.rem_euclid(360.0)) / 60.0;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r1, g1, b1) = if h < 1.0 {
        (c, x, 0.0)
    } else if h < 2.0 {
        (x, c, 0.0)
    } else if h < 3.0 {
        (0.0, c, x)
    } else if h < 4.0 {
        (0.0, x, c)
    } else if h < 5.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    let m = l - c * 0.5;
    [
        ((r1 + m).clamp(0.0, 1.0) * 255.0).round() as u8,
        ((g1 + m).clamp(0.0, 1.0) * 255.0).round() as u8,
        ((b1 + m).clamp(0.0, 1.0) * 255.0).round() as u8,
    ]
}
