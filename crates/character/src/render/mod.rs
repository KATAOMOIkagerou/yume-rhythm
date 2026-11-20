//! 分层 PNG 角色的渲染器（`CharacterRenderer` 的第一种实现）。

mod blend;
mod face;
mod fx;
mod runs;

use crate::CharacterRenderer;
use crate::anim::blink::{Blink, BlinkTiming};
use crate::anim::easing::{lerp, ping_pong};
use crate::anim::notes::{NoteFrame, Notes};
use crate::anim::{Pose, Rig};
use crate::frame::Frame;
use crate::pack::Pack;
use crate::pack::manifest::{LayerEffect, RigName, Slot};
use runs::{Run, RunCache};
use std::collections::HashMap;
use tiny_skia::{Pixmap, Transform};
use yume_beat::MotionFrame;
use yume_core::derive::PetScene;
use yume_core::{Random, SplitMix64};

const RING_DEFAULT_MS: f64 = 620.0;
const FLOAT_PERIOD_MS: f64 = 2200.0;
/// 特效上下漂浮 6 px，放大到 1.03 倍。
const FLOAT_RISE_PX: f64 = 6.0;
const FLOAT_GROW: f64 = 0.03;
/// 判断"画面没变"时的位置精度：1/4 像素。
const POS_STEPS: f64 = 4.0;

pub struct LayeredCharacter {
    pack: Pack,
    runs: Vec<Run>,
    caches: Vec<RunCache>,
    /// 发光图层的光晕，及其在画布上的左上角。
    glows: HashMap<String, (Pixmap, f32, f32)>,
    note_art: fx::NoteArt,
    rig: Rig,
    blink: Blink,
    notes: Notes,
    rng: SplitMix64,
    now: f64,
    scene: Option<PetScene>,
    pose: Pose,
    blinking: bool,
    note_frames: Vec<NoteFrame>,
    last_key: Option<FrameKey>,
}

impl LayeredCharacter {
    #[must_use]
    pub fn new(pack: Pack, seed: u64) -> Self {
        let runs = runs::plan(&pack.manifest.layers);
        let caches = vec![RunCache::default(); runs.len()];
        let u = 1.0 / pack.manifest.display_scale;
        let glows = pack
            .manifest
            .layers
            .iter()
            .filter(|l| l.effect == Some(LayerEffect::Glow))
            .filter_map(|l| l.image.as_ref())
            .filter_map(|name| {
                let sprite = pack.images.get(name)?;
                let (halo, pad) = fx::glow_halo(&sprite.pixmap, u);
                #[allow(clippy::cast_precision_loss, reason = "图片不超过 4096")]
                let (x, y) = (
                    (sprite.x - pad as i32) as f32,
                    (sprite.y - pad as i32) as f32,
                );
                Some((name.clone(), (halo, x, y)))
            })
            .collect();
        let note_art = fx::NoteArt::prepare(&note_masks(&pack), u);
        Self {
            pack,
            runs,
            caches,
            glows,
            note_art,
            rig: Rig::default(),
            blink: Blink::default(),
            notes: Notes::default(),
            rng: SplitMix64::new(seed),
            now: 0.0,
            scene: None,
            pose: Pose::default(),
            blinking: false,
            note_frames: Vec::new(),
            last_key: None,
        }
    }

    #[must_use]
    pub fn pack(&self) -> &Pack {
        &self.pack
    }

    fn px_to_canvas(&self) -> f32 {
        1.0 / self.pack.manifest.display_scale
    }
}

fn note_masks(pack: &Pack) -> Vec<&Pixmap> {
    let names = pack
        .manifest
        .effects
        .notes
        .iter()
        .flat_map(|n| n.images.iter());
    names
        .filter_map(|n| pack.images.get(n))
        .map(|s| &s.pixmap)
        .collect()
}

impl CharacterRenderer for LayeredCharacter {
    fn base_size(&self) -> (f32, f32) {
        let m = &self.pack.manifest;
        (
            m.viewport.width * m.display_scale,
            m.viewport.height * m.display_scale,
        )
    }

    fn advance(&mut self, now_ms: f64, scene: &PetScene, motion: &MotionFrame) {
        let m = &self.pack.manifest;
        let allowed = face::blink_allowed(&self.pack, scene);
        self.blinking = m.blink.as_ref().is_some_and(|b| {
            let timing = BlinkTiming {
                min_interval_ms: f64::from(b.min_interval_ms),
                max_interval_ms: f64::from(b.max_interval_ms),
                duration_ms: f64::from(b.duration_ms),
            };
            self.blink.advance(now_ms, &timing, allowed, &mut self.rng)
        });
        let ring_ms = m
            .effects
            .beat_ring
            .as_ref()
            .map_or(RING_DEFAULT_MS, |r| f64::from(r.duration_ms));
        let step = self
            .rig
            .step(now_ms, scene, motion, &m.motion, ring_ms, &mut self.rng);
        if let Some(def) = &m.effects.notes {
            if step.beat && self.rng.next_f64() < f64::from(def.chance) {
                self.notes.spawn(now_ms, def, &mut self.rng);
            }
            self.note_frames = self.notes.frames(now_ms, def);
        }
        self.pose = step.pose;
        self.now = now_ms;
        if self.scene.as_ref() != Some(scene) {
            self.scene = Some(scene.clone());
        }
    }

    fn render(&mut self, frame: &mut Frame, scale: f32) -> bool {
        let m = &self.pack.manifest;
        let k = m.display_scale * scale;
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "窗口尺寸为正且有限"
        )]
        let (w, h) = (
            (m.viewport.width * k).round() as u32,
            (m.viewport.height * k).round() as u32,
        );
        let Some(scene) = &self.scene else {
            frame.reset(w, h);
            return true;
        };
        let base = Transform::from_scale(k, k).pre_translate(-m.viewport.x, -m.viewport.y);
        let base_inv =
            Transform::from_translate(m.viewport.x, m.viewport.y).pre_scale(1.0 / k, 1.0 / k);
        let u = self.px_to_canvas();

        // 第一步：算出每一部分画在哪里，顺便得到这一帧的签名。
        let face = face::resolve(&self.pack, scene, self.blinking);
        let head = rig_transform(
            m.rigs.head.pivot,
            0.0,
            self.pose.head_y,
            self.pose.head_rotation,
            (self.pose.head_scale_x, self.pose.head_scale_y),
            u,
        );
        let body = rig_transform(
            m.rigs.body.pivot,
            self.pose.body_x,
            self.pose.body_y,
            0.0,
            (self.pose.body_scale_x, self.pose.body_scale_y),
            u,
        );
        #[allow(clippy::cast_precision_loss, reason = "图片不超过 4096")]
        let full = face.full.as_ref().and_then(|name| {
            let sprite = self.pack.images.get(name)?;
            let t = base
                .pre_concat(head)
                .pre_translate(sprite.x as f32, sprite.y as f32);
            Some((name.clone(), t))
        });
        #[allow(clippy::cast_possible_truncation, reason = "不透明度 0–1")]
        let glow_strength = lerp(0.45, 0.88, ping_pong(self.now, fx::GLOW_PERIOD_MS)) as f32;

        let mut key = FrameKey {
            size: (w, h),
            full: full
                .as_ref()
                .map(|(name, t)| (name.clone(), quantize(*t, w.max(h)))),
            ring: self.pose.ring.map(|e| q(e, 512.0)),
            notes: self.note_frames.iter().map(note_key).collect(),
            ..FrameKey::default()
        };
        let mut placed = Vec::with_capacity(self.runs.len());
        for (run, cache) in self.runs.iter().zip(self.caches.iter_mut()) {
            let layers = &m.layers[run.layers.clone()];
            // 整图表情只保留特效槽位。
            let skip = face.full.is_some() && layers.iter().all(|l| l.slot != Some(Slot::Overlay));
            let images: Vec<String> = if skip {
                Vec::new()
            } else {
                layers
                    .iter()
                    .filter_map(|l| runs::resolve_layer(l, scene, &face))
                    .collect()
            };
            if images.is_empty() {
                key.runs.push(None);
                placed.push(None);
                continue;
            }
            cache.update(&self.pack, images.clone(), base, k);
            let mut local = match run.rig {
                RigName::Root => Transform::identity(),
                RigName::Body => body,
                RigName::Head => head,
            };
            if run.effect == Some(LayerEffect::Float) {
                local = local.pre_concat(float_transform(self.now, m, u));
            }
            let glow = (run.effect == Some(LayerEffect::Glow)).then(|| base.pre_concat(local));
            if glow.is_some() {
                key.glow = q(f64::from(glow_strength), 64.0);
            }
            let out = base.pre_concat(local).pre_concat(base_inv);
            key.runs.push(Some((images, quantize(out, w.max(h)))));
            placed.push(Some((out, glow)));
        }

        // 与上一帧看起来一样（各部分位移都不到 1/4 像素）就不重画，省下合成和提交。
        // 空闲时的呼吸幅度只有一两个像素，这样大约一半的帧可以跳过。
        if self.last_key.as_ref() == Some(&key) && frame.width() == w && frame.height() == h {
            return false;
        }

        // 第二步：画。
        let target = frame.reset(w, h);
        if let (Some(e), Some(ring)) = (self.pose.ring, &m.effects.beat_ring) {
            fx::draw_ring(target, ring, e, base);
        }
        if let Some((name, t)) = &full
            && let Some(sprite) = self.pack.images.get(name)
        {
            blend::draw_affine(target, &sprite.pixmap, *t, 1.0);
        }
        for ((run, cache), place) in self.runs.iter().zip(&self.caches).zip(&placed) {
            let (Some((out, glow)), Some((pixmap, ox, oy))) = (place, &cache.image) else {
                continue;
            };
            if let Some(to_out) = glow {
                let layers = &m.layers[run.layers.clone()];
                draw_glow(target, &self.glows, layers, *to_out, glow_strength);
            }
            blit(target, pixmap, *ox, *oy, *out);
        }
        let masks = note_masks(&self.pack);
        if let Some(def) = &m.effects.notes {
            self.note_art
                .draw(target, &masks, def, &self.note_frames, base, u);
        }
        self.last_key = Some(key);
        true
    }
}

/// 一帧画面的签名：用到的图片，以及各部分量化后的位置。签名相同就不必重画。
#[derive(Clone, Debug, Default, PartialEq)]
struct FrameKey {
    size: (u32, u32),
    full: Option<(String, [i32; 6])>,
    runs: Vec<Option<(Vec<String>, [i32; 6])>>,
    glow: i32,
    ring: Option<i32>,
    notes: Vec<[i32; 6]>,
}

#[allow(clippy::cast_possible_truncation, reason = "量化后的值很小")]
fn q(v: f64, steps: f64) -> i32 {
    (v * steps).round() as i32
}

/// 量化到 1/4 像素：平移直接量化；缩放、旋转按画面尺寸换算成边缘处的位移再量化。
fn quantize(t: Transform, size: u32) -> [i32; 6] {
    let lin_steps = f64::from(size.max(1)) * POS_STEPS;
    let lin = |v: f32| q(f64::from(v), lin_steps);
    let pos = |v: f32| q(f64::from(v), POS_STEPS);
    [
        lin(t.sx),
        lin(t.kx),
        lin(t.ky),
        lin(t.sy),
        pos(t.tx),
        pos(t.ty),
    ]
}

fn note_key(n: &NoteFrame) -> [i32; 6] {
    [
        i32::try_from(n.image).unwrap_or(i32::MAX),
        q(n.x, 16.0),
        q(n.y, 16.0),
        q(n.rotation_deg, 16.0),
        q(n.scale, 1024.0),
        q(n.opacity, 256.0) ^ (q(n.hue, 1.0) << 9),
    ]
}

/// CSS 变换：以 `pivot` 为原点，`translate(x, y) rotate(r) scale(sx, sy)`。位移单位为屏幕像素。
fn rig_transform(
    pivot: [f32; 2],
    x: f64,
    y: f64,
    rot: f64,
    scale: (f64, f64),
    u: f32,
) -> Transform {
    #[allow(clippy::cast_possible_truncation, reason = "动画数值很小")]
    let (x, y, rot, sx, sy) = (
        x as f32 * u,
        y as f32 * u,
        rot as f32,
        scale.0 as f32,
        scale.1 as f32,
    );
    Transform::from_translate(pivot[0] + x, pivot[1] + y)
        .pre_rotate(rot)
        .pre_scale(sx, sy)
        .pre_translate(-pivot[0], -pivot[1])
}

/// 特效漂浮：以画布中心为原点上移并略微放大。
fn float_transform(now: f64, m: &crate::pack::manifest::Manifest, u: f32) -> Transform {
    let e = ping_pong(now, FLOAT_PERIOD_MS);
    #[allow(clippy::cast_precision_loss, reason = "画布不超过 4096")]
    let (cx, cy) = (m.canvas.width as f32 / 2.0, m.canvas.height as f32 / 2.0);
    #[allow(clippy::cast_possible_truncation, reason = "动画数值很小")]
    let (rise, grow) = (
        (-FLOAT_RISE_PX * e) as f32 * u,
        (1.0 + FLOAT_GROW * e) as f32,
    );
    Transform::from_translate(cx, cy + rise)
        .pre_scale(grow, grow)
        .pre_translate(-cx, -cy)
}

/// 发光图层先画光晕，强度随呼吸节奏在 45%–88% 之间变化。
fn draw_glow(
    target: &mut Pixmap,
    glows: &HashMap<String, (Pixmap, f32, f32)>,
    layers: &[crate::pack::manifest::LayerDef],
    to_out: Transform,
    strength: f32,
) {
    for layer in layers {
        let Some((halo, x, y)) = layer.image.as_ref().and_then(|n| glows.get(n)) else {
            continue;
        };
        blend::draw_affine(target, halo, to_out.pre_translate(*x, *y), strength);
    }
}

/// 不动的部位整数位置直接贴；动的部位按变换双线性采样。
fn blit(target: &mut Pixmap, pixmap: &Pixmap, x: i32, y: i32, out: Transform) {
    if out.is_identity() {
        blend::blit_over(target, pixmap, x, y);
    } else {
        #[allow(clippy::cast_precision_loss, reason = "输出坐标")]
        let t = out.pre_translate(x as f32, y as f32);
        blend::draw_affine(target, pixmap, t, 1.0);
    }
}
