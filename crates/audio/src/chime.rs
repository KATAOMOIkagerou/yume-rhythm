//! 倒计时结束的提示音：三个上行音（660、880、1046 Hz）。

const TONES: [f32; 3] = [660.0, 880.0, 1046.0];
const SPACING_S: f32 = 0.22;
const ATTACK_S: f32 = 0.02;
const DECAY_END_S: f32 = 0.4;
const STOP_S: f32 = 0.42;
const PEAK: f32 = 0.18;
const FLOOR: f32 = 0.001;

/// 生成单声道提示音采样。
#[must_use]
pub fn render(sample_rate: u32) -> Vec<f32> {
    #[allow(clippy::cast_precision_loss, reason = "采样率很小")]
    let rate = sample_rate.max(1) as f32;
    let total_s = SPACING_S * 2.0 + STOP_S;
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "时长为正且很短"
    )]
    let len = (total_s * rate).ceil() as usize;
    let mut out = vec![0.0; len];
    for (n, freq) in TONES.iter().enumerate() {
        #[allow(clippy::cast_precision_loss, reason = "下标很小")]
        let start = n as f32 * SPACING_S;
        for (i, sample) in out.iter_mut().enumerate() {
            #[allow(clippy::cast_precision_loss, reason = "下标很小")]
            let t = i as f32 / rate - start;
            if (0.0..STOP_S).contains(&t) {
                *sample += envelope(t) * (2.0 * std::f32::consts::PI * freq * t).sin();
            }
        }
    }
    out
}

/// 线性上升到峰值，再指数衰减到底（Web Audio 的 linearRamp + exponentialRamp）。
fn envelope(t: f32) -> f32 {
    if t < ATTACK_S {
        PEAK * t / ATTACK_S
    } else if t < DECAY_END_S {
        let k = (t - ATTACK_S) / (DECAY_END_S - ATTACK_S);
        PEAK * (FLOOR / PEAK).powf(k)
    } else {
        FLOOR
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn has_expected_length_and_peak() {
        let s = render(48_000);
        assert_eq!(s.len(), (0.86_f32 * 48_000.0).ceil() as usize);
        let peak = s.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(peak > 0.15 && peak < 0.25, "{peak}");
    }
}
