//! character.json 的数据结构（docs/角色包格式.md 第 1、2、4 节）。

use serde::Deserialize;
use yume_core::model::events::ActivityMode;

pub const FORMAT: u32 = 2;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub format: u32,
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub author: String,
    pub canvas: Size,
    pub viewport: Rect,
    pub display_scale: f32,
    pub rigs: Rigs,
    #[serde(default)]
    pub motion: Motion,
    pub layers: Vec<LayerDef>,
    #[serde(default)]
    pub blink: Option<BlinkDef>,
    #[serde(default)]
    pub effects: Effects,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rigs {
    pub body: RigDef,
    pub head: RigDef,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RigDef {
    pub pivot: [f32; 2],
}

/// 动作幅度倍率，1.0 为默认。
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Motion {
    pub nod_amplitude: f32,
    pub nod_tilt: f32,
    pub breath: f32,
    pub sway: f32,
}

impl Default for Motion {
    fn default() -> Self {
        Self {
            nod_amplitude: 1.0,
            nod_tilt: 1.0,
            breath: 1.0,
            sway: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum RigName {
    #[default]
    Root,
    Body,
    Head,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Slot {
    Eyes,
    Mouth,
    Overlay,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LayerEffect {
    /// 柔和发光脉动。
    Glow,
    /// 上下漂浮。
    Float,
}

/// 显示条件，全部满足才显示。
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Condition {
    pub activity: Option<ActivityMode>,
    pub music: Option<bool>,
    pub timer: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayerDef {
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub slot: Option<Slot>,
    #[serde(default)]
    pub rig: RigName,
    #[serde(default)]
    pub when: Condition,
    #[serde(default)]
    pub effect: Option<LayerEffect>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlinkDef {
    pub image: String,
    #[serde(default = "default_blink_min")]
    pub min_interval_ms: f32,
    #[serde(default = "default_blink_max")]
    pub max_interval_ms: f32,
    #[serde(default = "default_blink_duration")]
    pub duration_ms: f32,
}

fn default_blink_min() -> f32 {
    2400.0
}
fn default_blink_max() -> f32 {
    5200.0
}
fn default_blink_duration() -> f32 {
    150.0
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Effects {
    #[serde(default)]
    pub beat_ring: Option<RingDef>,
    #[serde(default)]
    pub notes: Option<NotesDef>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RingDef {
    pub center: [f32; 2],
    /// 起始与结束半径。
    pub radius: [f32; 2],
    /// 起始与结束线宽。
    pub width: [f32; 2],
    pub color: String,
    pub opacity: f32,
    pub duration_ms: f32,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotesDef {
    pub images: Vec<String>,
    pub area: Rect,
    /// 音符高度。
    pub size: f32,
    /// 上升距离。
    pub rise: f32,
    /// 左右漂移的最大幅度（总宽）。
    pub drift: f32,
    pub hue: [f32; 2],
    pub chance: f32,
    pub max: usize,
    pub duration_ms: f32,
}
