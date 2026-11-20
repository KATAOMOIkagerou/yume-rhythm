use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PointF32 {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestLayers {
    #[serde(default)]
    pub scene_bg: String,
    pub chair_back: String,
    pub body_base: String,
    pub hair_back: String,
    pub head_base: String,
    pub blush_base: String,
    pub hair_front: String,
    pub headphones: String,
    pub desk_surface: String,
    pub arms_chill: String,
    pub arms_study: String,
    pub computer_off: String,
    pub computer_on: String,
    pub books: String,
    pub timer_clock: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FacePair {
    pub eyes: String,
    pub mouth: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestExpressions {
    pub neutral: FacePair,
    pub happy: FacePair,
    pub sad: FacePair,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RandomEventAssets {
    pub eyes: String,
    pub mouth: String,
    #[serde(default)]
    pub fx: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestRandomEvents {
    pub doze: RandomEventAssets,
    pub think: RandomEventAssets,
    pub shock: RandomEventAssets,
    pub love: RandomEventAssets,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterManifest {
    pub pack_name: String,
    pub version: String,
    pub canvas_width: u32,
    pub canvas_height: u32,
    pub head_pivot: PointF32,
    #[serde(default = "default_one")]
    pub nod_amplitude_scale: f32,
    #[serde(default = "default_one")]
    pub nod_tilt_scale: f32,
    #[serde(default)]
    pub show_scene_bg: bool,
    #[serde(default = "default_true")]
    pub show_music_headphones: bool,
    pub layers: ManifestLayers,
    pub expressions: ManifestExpressions,
    pub blink_eyes: String,
    pub random_events: ManifestRandomEvents,
}

fn default_one() -> f32 {
    1.0
}

fn default_true() -> bool {
    true
}

impl CharacterManifest {
    pub fn parse_json(raw: &str) -> Result<Self, String> {
        let manifest: Self = serde_json::from_str(raw)
            .map_err(|e| format!("角色清单格式无效：{e}"))?;
        if manifest.canvas_width == 0 || manifest.canvas_height == 0 {
            return Err("角色清单画布尺寸无效".into());
        }
        if manifest.blink_eyes.trim().is_empty() {
            return Err("角色清单缺少必需的眨眼图层配置".into());
        }
        Ok(manifest)
    }

    /// Collects all referenced asset files except `sceneBg` (transparent desktop pet omits background).
    pub fn collect_required_files(&self) -> Vec<String> {
        let mut set = BTreeSet::new();
        let mut add = |s: &str| {
            let trimmed = s.trim();
            if !trimmed.is_empty() {
                set.insert(trimmed.to_string());
            }
        };

        add(&self.layers.chair_back);
        add(&self.layers.body_base);
        add(&self.layers.hair_back);
        add(&self.layers.head_base);
        add(&self.layers.blush_base);
        add(&self.layers.hair_front);
        add(&self.layers.headphones);
        add(&self.layers.desk_surface);
        add(&self.layers.arms_chill);
        add(&self.layers.arms_study);
        add(&self.layers.computer_off);
        add(&self.layers.computer_on);
        add(&self.layers.books);
        add(&self.layers.timer_clock);

        for pair in [
            &self.expressions.neutral,
            &self.expressions.happy,
            &self.expressions.sad,
        ] {
            add(&pair.eyes);
            add(&pair.mouth);
        }
        add(&self.blink_eyes);

        for ev in [
            &self.random_events.doze,
            &self.random_events.think,
            &self.random_events.shock,
            &self.random_events.love,
        ] {
            add(&ev.eyes);
            add(&ev.mouth);
            if let Some(fx) = &ev.fx {
                add(fx);
            }
        }

        set.into_iter().collect()
    }
}
