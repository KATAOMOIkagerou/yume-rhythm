//! 流式重采样：三次 Hermite 插值，交错立体声。用于把文件采样率转换到设备采样率。

/// 交错立体声重采样器。可以分块喂入，块与块之间连续。
pub struct Resampler {
    step: f64,
    /// 输出位置在输入流中的分数下标（相对 `history` 起点）。
    pos: f64,
    /// 最近 4 帧输入（左、右）。
    history: [[f32; 2]; 4],
    primed: usize,
}

impl Resampler {
    #[must_use]
    pub fn new(from_rate: u32, to_rate: u32) -> Self {
        Self {
            step: f64::from(from_rate.max(1)) / f64::from(to_rate.max(1)),
            pos: 0.0,
            history: [[0.0; 2]; 4],
            primed: 0,
        }
    }

    #[must_use]
    pub fn is_identity(&self) -> bool {
        (self.step - 1.0).abs() < f64::EPSILON
    }

    /// 处理一块交错立体声输入，把输出追加到 `out`。
    pub fn process(&mut self, input: &[f32], out: &mut Vec<f32>) {
        if self.is_identity() {
            out.extend_from_slice(input);
            return;
        }
        for &[left, right] in input.as_chunks::<2>().0 {
            self.history.rotate_left(1);
            self.history[3] = [left, right];
            if self.primed < 3 {
                self.primed += 1;
                continue;
            }
            // 在 history[1] 与 history[2] 之间插值，pos ∈ [0, 1)。
            while self.pos < 1.0 {
                #[allow(clippy::cast_possible_truncation, reason = "插值系数 0–1")]
                let t = self.pos as f32;
                for ch in 0..2 {
                    let p = [
                        self.history[0][ch],
                        self.history[1][ch],
                        self.history[2][ch],
                        self.history[3][ch],
                    ];
                    out.push(hermite(p, t));
                }
                self.pos += self.step;
            }
            self.pos -= 1.0;
        }
    }
}

fn hermite(p: [f32; 4], t: f32) -> f32 {
    let c0 = p[1];
    let c1 = 0.5 * (p[2] - p[0]);
    let c2 = p[0] - 2.5 * p[1] + 2.0 * p[2] - 0.5 * p[3];
    let c3 = 0.5 * (p[3] - p[0]) + 1.5 * (p[1] - p[2]);
    ((c3 * t + c2) * t + c1) * t + c0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_length_follows_rate_ratio() {
        let mut r = Resampler::new(44_100, 48_000);
        let input = vec![0.0; 44_100 * 2];
        let mut out = Vec::new();
        r.process(&input, &mut out);
        let frames = out.len() / 2;
        assert!((47_990..=48_000).contains(&frames), "{frames}");
    }

    #[test]
    fn reproduces_a_slow_sine() {
        let mut r = Resampler::new(44_100, 48_000);
        let freq = 440.0;
        let input: Vec<f32> = (0..4410)
            .flat_map(|i| {
                let v = (2.0 * std::f32::consts::PI * freq * i as f32 / 44_100.0).sin();
                [v, v]
            })
            .collect();
        let mut out = Vec::new();
        r.process(&input, &mut out);
        // 输出相对输入延迟一帧（插值需要前后各一帧）。
        for (k, pair) in out
            .as_chunks::<2>()
            .0
            .iter()
            .enumerate()
            .skip(10)
            .take(1000)
        {
            let t = (k as f32) * 44_100.0 / 48_000.0 + 1.0;
            let expected = (2.0 * std::f32::consts::PI * freq * t / 44_100.0).sin();
            assert!((pair[0] - expected).abs() < 0.01, "frame {k}");
        }
    }

    #[test]
    fn identity_passes_through() {
        let mut r = Resampler::new(48_000, 48_000);
        let mut out = Vec::new();
        r.process(&[0.1, 0.2, 0.3, 0.4], &mut out);
        assert_eq!(out, vec![0.1, 0.2, 0.3, 0.4]);
    }
}
