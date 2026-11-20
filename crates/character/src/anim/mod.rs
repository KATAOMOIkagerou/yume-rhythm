//! 角色动作：节拍弹跳、锁拍点头、呼吸摇摆、打瞌睡下垂、节拍光环（功能清单第 8 节）。
//!
//! 物理模型每帧推进一次。输出单位是**屏幕像素**
//! （缩放 100% 时），渲染时再按角色包的 `display_scale` 换算成画布像素。

pub mod blink;
pub mod easing;
pub mod notes;

use crate::pack::manifest::Motion;
use yume_beat::{MotionFrame, locked_beat_nod};
use yume_core::Random;
use yume_core::derive::PetScene;

/// 物理参数按 60 fps 的帧间隔标定。
const FRAME_MS: f64 = 16.667;
/// 单帧最大步长，避免卡顿后弹簧爆冲。
const MAX_DT_MS: f64 = 48.0;
/// 点头位移放大 1.45 倍显示。
const NOD_VISUAL_SCALE: f64 = 1.45;

/// 某一帧的姿态。位移单位为屏幕像素，角度单位为度。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pose {
    pub head_y: f64,
    pub head_rotation: f64,
    pub head_scale_x: f64,
    pub head_scale_y: f64,
    pub body_x: f64,
    pub body_y: f64,
    pub body_scale_x: f64,
    pub body_scale_y: f64,
    /// 节拍光环进度（已缓动，0–1）。`None` 表示不显示。
    pub ring: Option<f64>,
}

/// 本帧是否触发了一次节拍（用于生成音符）。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Step {
    pub pose: Pose,
    pub beat: bool,
}

#[derive(Clone, Debug)]
pub struct Rig {
    head_y: f64,
    head_vy: f64,
    head_rot: f64,
    head_vrot: f64,
    scale_pulse: f64,
    scale_v: f64,
    squash_y: f64,
    doze_blend: f64,
    hit_strength: f64,
    ring_progress: f64,
    prev_beats: Option<u32>,
    last_time: Option<f64>,
}

impl Default for Rig {
    fn default() -> Self {
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
            prev_beats: None,
            last_time: None,
        }
    }
}

impl Rig {
    /// 推进一帧。`ring_ms` 是光环扩散时长。
    pub fn step(
        &mut self,
        now: f64,
        scene: &PetScene,
        frame: &MotionFrame,
        motion: &Motion,
        ring_ms: f64,
        rng: &mut dyn Random,
    ) -> Step {
        let dt_ms = self
            .last_time
            .map_or(0.0, |t| (now - t).clamp(0.0, MAX_DT_MS));
        self.last_time = Some(now);
        let dt = dt_ms / FRAME_MS;
        let idle = !scene.follow_beat;

        let prev = self.prev_beats.replace(frame.beats).unwrap_or(frame.beats);
        let beat = !idle && frame.beats > prev;
        if beat {
            let strength = 0.85 + frame.strength.min(2.0) * 1.15;
            self.hit_strength = strength;
            self.head_vy += strength * 1.2;
            self.head_vrot += (rng.next_f64() - 0.4) * strength * 0.6;
            self.scale_v += strength * 0.09;
            self.ring_progress = 0.0;
        }

        let level = frame.level;
        let level = if idle && level <= 0.03 { 0.1 } else { level };

        // 竖直弹簧；锁拍时由预测拍位置直接给出点头曲线。
        self.head_vy += -0.115 * self.head_y * dt;
        self.head_vy *= 0.86f64.powf(dt);
        self.head_y += self.head_vy * dt;
        let locked_y = if !idle && frame.locked {
            locked_beat_nod(now, frame.next_beat_at, frame.period_ms, self.hit_strength)
        } else {
            None
        };
        if let Some(y) = locked_y {
            self.head_y = y;
            self.head_vy = 0.0;
        }

        // 歪头与缩放脉冲。
        self.head_vrot += -0.09 * self.head_rot * dt;
        self.head_vrot *= 0.9f64.powf(dt);
        self.head_rot += self.head_vrot * dt;
        self.scale_v += -0.22 * (self.scale_pulse - 1.0) * dt;
        self.scale_v *= 0.84f64.powf(dt);
        self.scale_pulse = (self.scale_pulse + self.scale_v * dt).clamp(0.6, 1.6);
        let target_squash = 1.0 - (self.head_y * 0.004).clamp(-0.1, 0.1);
        self.squash_y += (target_squash - self.squash_y) * (0.25 * dt).min(1.0);

        // 呼吸、摇摆、打瞌睡下垂。
        let breath = (now / 340.0).sin() * (1.1 + level * 2.6) * f64::from(motion.breath);
        let sway = (now / 520.0).sin() * (1.3 + level * 2.4) * f64::from(motion.sway);
        let target_doze = if idle && scene.dozing { 1.0 } else { 0.0 };
        self.doze_blend += (target_doze - self.doze_blend) * (0.06 * dt).min(1.0);
        let doze_y = self.doze_blend * (9.5 + (now / 650.0).sin() * 3.5);
        let doze_rot = self.doze_blend * (-3.2 + (now / 900.0).sin() * 1.6);

        let nod = NOD_VISUAL_SCALE * f64::from(motion.nod_amplitude);
        let pulse = (self.scale_pulse - 1.0).max(0.0) * 0.03;

        let ring = if self.ring_progress < 1.0 {
            self.ring_progress = (self.ring_progress + dt_ms / ring_ms.max(1.0)).min(1.0);
            Some(1.0 - (1.0 - self.ring_progress).powi(3))
        } else {
            None
        };

        let pose = Pose {
            head_y: self.head_y * nod + breath + doze_y,
            head_rotation: self.head_rot * f64::from(motion.nod_tilt) + doze_rot,
            head_scale_x: 2.0 - self.squash_y + pulse,
            head_scale_y: self.squash_y + pulse,
            body_x: sway * 0.65,
            body_y: -self.head_y * 0.22 * nod + breath * 0.4,
            body_scale_x: self.squash_y,
            body_scale_y: 2.0 - self.squash_y,
            ring,
        };
        Step { pose, beat }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yume_core::SplitMix64;
    use yume_core::model::events::ActivityMode;

    fn scene(follow: bool, dozing: bool) -> PetScene {
        PetScene {
            activity: ActivityMode::Chill,
            music: follow,
            follow_beat: follow,
            timer_running: false,
            base_expression: "neutral".into(),
            accessory_expression: None,
            event: None,
            dozing,
        }
    }

    fn run(rig: &mut Rig, scene: &PetScene, frames: &[MotionFrame], from: f64) -> Vec<Step> {
        let mut rng = SplitMix64::new(4);
        let motion = Motion::default();
        frames
            .iter()
            .enumerate()
            .map(|(i, f)| {
                rig.step(
                    from + i as f64 * FRAME_MS,
                    scene,
                    f,
                    &motion,
                    620.0,
                    &mut rng,
                )
            })
            .collect()
    }

    #[test]
    fn beat_kicks_head_and_ring_only_when_following() {
        let quiet = MotionFrame::default();
        let hit = MotionFrame {
            beats: 1,
            strength: 1.0,
            ..MotionFrame::default()
        };
        let steps = run(
            &mut Rig::default(),
            &scene(true, false),
            &[quiet, hit, hit, hit],
            0.0,
        );
        assert!(steps[1].beat && steps[1].pose.ring.is_some());
        assert!(steps[3].pose.head_y > steps[0].pose.head_y);

        let steps = run(
            &mut Rig::default(),
            &scene(false, false),
            &[quiet, hit],
            0.0,
        );
        assert!(!steps[1].beat && steps[1].pose.ring.is_none());
    }

    #[test]
    fn dozing_droops_head_gradually() {
        let frames = vec![MotionFrame::default(); 240];
        let steps = run(&mut Rig::default(), &scene(false, true), &frames, 0.0);
        let awake = run(&mut Rig::default(), &scene(false, false), &frames, 0.0);
        let last = steps.len() - 1;
        assert!(steps[last].pose.head_y > awake[last].pose.head_y + 4.0);
        assert!(steps[last].pose.head_rotation < awake[last].pose.head_rotation);
    }
}
