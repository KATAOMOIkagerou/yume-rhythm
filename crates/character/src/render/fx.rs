//! 特效绘制：节拍光环、音符、发光。

use super::blend::{draw_affine, stroke_ring};
use crate::anim::easing::lerp;
use crate::anim::notes::{NoteFrame, hsl_to_rgb};
use crate::pack::manifest::{NotesDef, RingDef};
use crate::sprite::{blurred_alpha, parse_hex, tinted};
use std::collections::HashMap;
use tiny_skia::{Pixmap, Transform};

/// 光环：随进度 `e`（已缓动）扩散变淡。`base` 为画布到输出的变换（只含缩放与平移）。
pub fn draw_ring(target: &mut Pixmap, ring: &RingDef, e: f64, base: Transform) {
    let radius = lerp(f64::from(ring.radius[0]), f64::from(ring.radius[1]), e);
    let width = lerp(f64::from(ring.width[0]), f64::from(ring.width[1]), e);
    let opacity = (1.0 - e) * f64::from(ring.opacity);
    let (r, g, b) = parse_hex(&ring.color).unwrap_or((192, 132, 252));
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "已限定在 0–255"
    )]
    let alpha = (opacity.clamp(0.0, 1.0) * 255.0).round() as u32;
    if alpha == 0 {
        return;
    }
    let mul = |c: u8| (u32::from(c) * alpha + 127) / 255;
    let color = mul(b) | (mul(g) << 8) | (mul(r) << 16) | (alpha << 24);
    let center = (
        base.sx * ring.center[0] + base.tx,
        base.sy * ring.center[1] + base.ty,
    );
    #[allow(clippy::cast_possible_truncation, reason = "画布尺寸")]
    let (radius, width) = (radius as f32 * base.sx, width as f32 * base.sx);
    stroke_ring(target, center, radius, width, color);
}

/// 音符：白色字形按色相着色，下方带白色光晕。着色结果按（图片, 色相）缓存。
#[derive(Default)]
pub struct NoteArt {
    halos: Vec<(Pixmap, u32)>,
    tinted: HashMap<(usize, u16), Pixmap>,
}

/// 音符的光晕：向下 2 px、模糊 6 px、白色 85%。
const HALO_OFFSET_PX: f32 = 2.0;
const HALO_BLUR_PX: f32 = 6.0;
const HALO_OPACITY: f32 = 0.85;

impl NoteArt {
    /// `masks` 为音符原图；`px_to_canvas` 把屏幕像素换算为画布像素。
    pub fn prepare(masks: &[&Pixmap], px_to_canvas: f32) -> Self {
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "模糊半径很小"
        )]
        let radius = ((HALO_BLUR_PX * px_to_canvas) / 2.0).round().max(1.0) as u32;
        let halos = masks
            .iter()
            .map(|m| {
                let (blur, pad) = blurred_alpha(m, radius);
                (tinted(&blur, (255, 255, 255)), pad)
            })
            .collect();
        Self {
            halos,
            tinted: HashMap::new(),
        }
    }

    pub fn draw(
        &mut self,
        target: &mut Pixmap,
        masks: &[&Pixmap],
        def: &NotesDef,
        notes: &[NoteFrame],
        base: Transform,
        px_to_canvas: f32,
    ) {
        for note in notes {
            let Some(mask) = masks.get(note.image) else {
                continue;
            };
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "色相 0–360"
            )]
            let hue_key = note.hue.round() as u16;
            let colored = self
                .tinted
                .entry((note.image, hue_key))
                .or_insert_with(|| tinted(mask, hsl_to_rgb(note.hue, 0.88, 0.68)));
            #[allow(clippy::cast_precision_loss, reason = "图片不超过 4096")]
            let (w, h) = (mask.width() as f32, mask.height() as f32);
            #[allow(clippy::cast_possible_truncation, reason = "画布坐标与小角度")]
            let place = base
                .pre_translate(note.x as f32, note.y as f32)
                .pre_rotate(note.rotation_deg as f32)
                .pre_scale(
                    def.size / h * note.scale as f32,
                    def.size / h * note.scale as f32,
                )
                .pre_translate(-w / 2.0, -h / 2.0);
            #[allow(clippy::cast_possible_truncation, reason = "不透明度 0–1")]
            let opacity = note.opacity.clamp(0.0, 1.0) as f32;
            if opacity <= 0.0 {
                continue;
            }
            if let Some((halo, pad)) = self.halos.get(note.image) {
                #[allow(clippy::cast_precision_loss, reason = "边宽很小")]
                let shift = -(*pad as f32);
                let t = place
                    .pre_translate(shift, shift + HALO_OFFSET_PX * px_to_canvas * h / def.size);
                draw_affine(target, halo, t, opacity * HALO_OPACITY);
            }
            draw_affine(target, colored, place, opacity);
        }
        // 色相是连续随机值，缓存会慢慢增长；超过上限就整体清空。
        if self.tinted.len() > 512 {
            self.tinted.clear();
        }
    }
}

/// 发光图层的光晕：图片 alpha 模糊后着粉色，模糊半径 6–15 px。
pub const GLOW_RGB: (u8, u8, u8) = (255, 140, 190);
pub const GLOW_PERIOD_MS: f64 = 2400.0;
const GLOW_BLUR_PX: f32 = 10.0;

#[must_use]
pub fn glow_halo(sprite: &Pixmap, px_to_canvas: f32) -> (Pixmap, u32) {
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "模糊半径很小"
    )]
    let radius = ((GLOW_BLUR_PX * px_to_canvas) / 2.0).round().max(1.0) as u32;
    let (blur, pad) = blurred_alpha(sprite, radius);
    (tinted(&blur, GLOW_RGB), pad)
}
