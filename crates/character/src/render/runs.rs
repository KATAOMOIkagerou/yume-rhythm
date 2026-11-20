//! 图层分组与缓存。
//!
//! 同一部位、没有特效的连续图层合成一组（run），按输出分辨率预先画好缓存，
//! 只有组内图片变化（换表情、眨眼、切换道具）或缩放变化时才重画。
//! 每帧只需把几张缓存图按部位的变换贴上去：不动的部位是整数位置直接贴，几乎没有开销。

use crate::pack::Pack;
use crate::pack::manifest::{Condition, LayerDef, LayerEffect, RigName, Slot};
use crate::render::face::Face;
use tiny_skia::{FilterQuality, Pixmap, PixmapPaint, Transform};
use yume_core::derive::PetScene;

/// 一组连续图层。带特效的图层单独成组。
#[derive(Clone, Debug)]
pub struct Run {
    pub rig: RigName,
    pub effect: Option<LayerEffect>,
    pub layers: std::ops::Range<usize>,
}

/// 把图层切分成组。
#[must_use]
pub fn plan(layers: &[LayerDef]) -> Vec<Run> {
    let mut runs: Vec<Run> = Vec::new();
    for (i, layer) in layers.iter().enumerate() {
        let joinable = layer.effect.is_none();
        match runs.last_mut() {
            Some(run) if joinable && run.effect.is_none() && run.rig == layer.rig => {
                run.layers.end = i + 1;
            }
            _ => runs.push(Run {
                rig: layer.rig,
                effect: layer.effect,
                layers: i..i + 1,
            }),
        }
    }
    runs
}

/// 某图层此刻显示哪张图。条件不满足或槽位为空时为 `None`。
#[must_use]
pub fn resolve_layer(layer: &LayerDef, scene: &PetScene, face: &Face) -> Option<String> {
    if !condition_met(&layer.when, scene) {
        return None;
    }
    match (&layer.image, layer.slot) {
        (Some(image), _) => Some(image.clone()),
        (None, Some(Slot::Eyes)) => face.eyes.clone(),
        (None, Some(Slot::Mouth)) => face.mouth.clone(),
        (None, Some(Slot::Overlay)) => face.overlay.clone(),
        (None, None) => None,
    }
}

fn condition_met(c: &Condition, scene: &PetScene) -> bool {
    c.activity.is_none_or(|a| a == scene.activity)
        && c.music.is_none_or(|m| m == scene.music)
        && c.timer.is_none_or(|t| t == scene.timer_running)
}

/// 一组图层的缓存：`pixmap` 左上角位于输出坐标 `(x, y)`（未施加部位变换时）。
#[derive(Clone, Debug, Default)]
pub struct RunCache {
    key: Vec<String>,
    scale: f32,
    pub image: Option<(Pixmap, i32, i32)>,
}

impl RunCache {
    /// 组内图片或缩放变化时重画。`base` 是画布到输出的变换（缩放 `k` 加视口平移）。
    pub fn update(&mut self, pack: &Pack, images: Vec<String>, base: Transform, k: f32) {
        if self.key == images && (self.scale - k).abs() < f32::EPSILON && self.image.is_some() {
            return;
        }
        self.image = compose(pack, &images, base);
        self.key = images;
        self.scale = k;
    }
}

fn compose(pack: &Pack, images: &[String], base: Transform) -> Option<(Pixmap, i32, i32)> {
    let sprites: Vec<_> = images
        .iter()
        .filter_map(|name| pack.images.get(name))
        .collect();
    // 求输出坐标下的包围盒，四周多留 1 像素给双线性采样。
    let mut bounds: Option<(f32, f32, f32, f32)> = None;
    for s in &sprites {
        #[allow(clippy::cast_precision_loss, reason = "图片不超过 4096")]
        let (x0, y0) = (s.x as f32, s.y as f32);
        #[allow(clippy::cast_precision_loss, reason = "图片不超过 4096")]
        let (x1, y1) = (x0 + s.pixmap.width() as f32, y0 + s.pixmap.height() as f32);
        let (ax, ay) = map(base, x0, y0);
        let (bx, by) = map(base, x1, y1);
        let b = (ax.min(bx), ay.min(by), ax.max(bx), ay.max(by));
        bounds = Some(match bounds {
            None => b,
            Some(o) => (o.0.min(b.0), o.1.min(b.1), o.2.max(b.2), o.3.max(b.3)),
        });
    }
    let (x0, y0, x1, y1) = bounds?;
    #[allow(clippy::cast_possible_truncation, reason = "坐标在输出尺寸范围内")]
    let (ox, oy) = ((x0.floor() as i32) - 1, (y0.floor() as i32) - 1);
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "宽高为正"
    )]
    let (w, h) = (
        ((x1.ceil() as i32) + 1 - ox) as u32,
        ((y1.ceil() as i32) + 1 - oy) as u32,
    );
    let mut pixmap = Pixmap::new(w.max(1), h.max(1))?;
    #[allow(clippy::cast_precision_loss, reason = "坐标在输出尺寸范围内")]
    let to_local = Transform::from_translate(-ox as f32, -oy as f32).pre_concat(base);
    let paint = PixmapPaint {
        quality: FilterQuality::Bicubic,
        ..PixmapPaint::default()
    };
    for s in sprites {
        #[allow(clippy::cast_precision_loss, reason = "图片不超过 4096")]
        let t = to_local.pre_translate(s.x as f32, s.y as f32);
        pixmap.draw_pixmap(0, 0, s.pixmap.as_ref(), &paint, t, None);
    }
    crate::sprite::clamp_premultiplied(&mut pixmap);
    Some((pixmap, ox, oy))
}

fn map(t: Transform, x: f32, y: f32) -> (f32, f32) {
    (t.sx * x + t.kx * y + t.tx, t.ky * x + t.sy * y + t.ty)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layer(rig: RigName, effect: Option<LayerEffect>) -> LayerDef {
        LayerDef {
            image: Some("a.png".into()),
            slot: None,
            rig,
            when: Condition::default(),
            effect,
        }
    }

    #[test]
    fn groups_consecutive_layers_by_rig_and_isolates_effects() {
        use RigName::{Body, Head, Root};
        let layers = [
            layer(Root, None),
            layer(Root, None),
            layer(Body, None),
            layer(Head, None),
            layer(Head, None),
            layer(Root, Some(LayerEffect::Glow)),
            layer(Root, None),
        ];
        let runs: Vec<_> = plan(&layers).into_iter().map(|r| r.layers).collect();
        assert_eq!(runs, vec![0..2, 2..3, 3..5, 5..6, 6..7]);
    }
}
