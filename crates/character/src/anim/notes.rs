//! 节拍音符粒子（功能清单第 8 节）。

use super::easing::{NOTE_FLOAT, lerp};
use crate::pack::manifest::NotesDef;
use yume_core::Random;

/// 关键帧以 74 px 上升为基准，其余位移按比例换算。
const RISE_START: f64 = -8.0 / 74.0;
const RISE_MID: f64 = 14.0 / 74.0;
const SPLIT: f64 = 0.18;

#[derive(Clone, Debug)]
struct Note {
    born: f64,
    image: usize,
    x: f64,
    y: f64,
    hue: f64,
    drift: f64,
}

/// 某一时刻的音符外观。坐标为画布像素，`(x, y)` 是音符中心。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NoteFrame {
    pub image: usize,
    pub x: f64,
    pub y: f64,
    pub scale: f64,
    pub rotation_deg: f64,
    pub opacity: f64,
    pub hue: f64,
}

#[derive(Clone, Debug, Default)]
pub struct Notes {
    live: Vec<Note>,
}

impl Notes {
    pub fn spawn(&mut self, now: f64, def: &NotesDef, rng: &mut dyn Random) {
        if self.live.len() >= def.max {
            let excess = self.live.len() + 1 - def.max;
            self.live.drain(..excess);
        }
        let area = &def.area;
        let note = Note {
            born: now,
            image: rng.pick(def.images.len()).unwrap_or(0),
            x: f64::from(area.x) + rng.next_f64() * f64::from(area.width),
            y: f64::from(area.y) + rng.next_f64() * f64::from(area.height),
            hue: lerp(f64::from(def.hue[0]), f64::from(def.hue[1]), rng.next_f64()),
            drift: (rng.next_f64() - 0.5) * f64::from(def.drift),
        };
        self.live.push(note);
    }

    /// 移除过期音符，返回当前每个音符的外观。
    pub fn frames(&mut self, now: f64, def: &NotesDef) -> Vec<NoteFrame> {
        let duration = f64::from(def.duration_ms);
        self.live.retain(|n| now - n.born < duration);
        let rise = f64::from(def.rise);
        self.live
            .iter()
            .map(|n| {
                let t = ((now - n.born) / duration).clamp(0.0, 1.0);
                let (opacity, tx, ty, scale, rot) = if t < SPLIT {
                    let e = NOTE_FLOAT.apply(t / SPLIT);
                    (
                        e,
                        lerp(0.0, n.drift * 0.25, e),
                        lerp(-RISE_START * rise, -RISE_MID * rise, e),
                        lerp(0.65, 1.08, e),
                        lerp(-8.0, 6.0, e),
                    )
                } else {
                    let e = NOTE_FLOAT.apply((t - SPLIT) / (1.0 - SPLIT));
                    (
                        1.0 - e,
                        lerp(n.drift * 0.25, n.drift, e),
                        lerp(-RISE_MID * rise, -rise, e),
                        lerp(1.08, 0.92, e),
                        lerp(6.0, -10.0, e),
                    )
                };
                NoteFrame {
                    image: n.image,
                    x: n.x + tx,
                    y: n.y + ty,
                    scale,
                    rotation_deg: rot,
                    opacity,
                    hue: n.hue,
                }
            })
            .collect()
    }

    pub fn clear(&mut self) {
        self.live.clear();
    }
}

/// HSL 转 RGB（各分量 0–1 输入，输出 0–255）。
#[must_use]
pub fn hsl_to_rgb(h_deg: f64, s: f64, l: f64) -> (u8, u8, u8) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h_deg.rem_euclid(360.0) / 60.0;
    let x = c * (1.0 - (hp.rem_euclid(2.0) - 1.0).abs());
    let (r, g, b) = match hp {
        h if h < 1.0 => (c, x, 0.0),
        h if h < 2.0 => (x, c, 0.0),
        h if h < 3.0 => (0.0, c, x),
        h if h < 4.0 => (0.0, x, c),
        h if h < 5.0 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "已限定在 0–255"
    )]
    let to8 = |v: f64| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    (to8(r), to8(g), to8(b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pack::manifest::Rect;
    use yume_core::SplitMix64;

    fn def() -> NotesDef {
        NotesDef {
            images: vec!["a.png".into()],
            area: Rect {
                x: 0.0,
                y: 100.0,
                width: 10.0,
                height: 10.0,
            },
            size: 36.0,
            rise: 110.0,
            drift: 50.0,
            hue: [190.0, 320.0],
            chance: 0.85,
            max: 3,
            duration_ms: 1500.0,
        }
    }

    #[test]
    fn notes_rise_fade_and_expire() {
        let mut notes = Notes::default();
        let mut rng = SplitMix64::new(2);
        notes.spawn(0.0, &def(), &mut rng);
        let start = notes.frames(0.0, &def())[0];
        assert!(start.opacity.abs() < 1e-9);
        let mid = notes.frames(600.0, &def())[0];
        assert!(mid.opacity > 0.0 && mid.y < start.y);
        assert!(notes.frames(1500.0, &def()).is_empty());
    }

    #[test]
    fn keeps_at_most_max() {
        let mut notes = Notes::default();
        let mut rng = SplitMix64::new(2);
        for i in 0..10 {
            notes.spawn(f64::from(i), &def(), &mut rng);
        }
        assert_eq!(notes.frames(10.0, &def()).len(), 3);
    }

    #[test]
    fn hsl_matches_css() {
        assert_eq!(hsl_to_rgb(0.0, 1.0, 0.5), (255, 0, 0));
        assert_eq!(hsl_to_rgb(240.0, 1.0, 0.5), (0, 0, 255));
    }
}
