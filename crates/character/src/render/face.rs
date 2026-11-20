//! 由场景与眨眼状态决定眼、嘴、特效用哪张图。

use crate::pack::Pack;
use yume_core::derive::PetScene;
use yume_core::model::events::RandomEvent;
use yume_core::model::expression::NEUTRAL;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Face {
    pub eyes: Option<String>,
    pub mouth: Option<String>,
    pub overlay: Option<String>,
    /// 基础表情的整图：有它时只画这张图（跟随头部）和特效。
    pub full: Option<String>,
}

/// 此刻是否允许眨眼。
#[must_use]
pub fn blink_allowed(pack: &Pack, scene: &PetScene) -> bool {
    let Some(base) = pack.expression(&scene.base_expression) else {
        return false;
    };
    let accessory_has_eyes = scene
        .accessory_expression
        .as_deref()
        .and_then(|id| pack.expression(id))
        .is_some_and(|a| a.resources.eyes.is_some());
    pack.manifest.blink.is_some()
        && base.blink
        && base.resources.animation.is_none()
        && !accessory_has_eyes
        && !matches!(scene.event, Some(RandomEvent::Love | RandomEvent::Doze))
}

#[must_use]
pub fn resolve(pack: &Pack, scene: &PetScene, blinking: bool) -> Face {
    let neutral = pack.expression(NEUTRAL).map(|e| &e.resources);
    let base = pack
        .expression(&scene.base_expression)
        .map(|e| &e.resources)
        .or(neutral);
    let accessory = scene
        .accessory_expression
        .as_deref()
        .and_then(|id| pack.expression(id))
        .map(|e| &e.resources);

    let base_eyes = base
        .and_then(|r| r.eyes.clone())
        .or_else(|| neutral.and_then(|r| r.eyes.clone()));
    let blink_eyes = pack.manifest.blink.as_ref().map(|b| b.image.clone());
    let eyes = accessory
        .and_then(|a| a.eyes.clone())
        .or(if blinking { blink_eyes } else { None })
        .or(base_eyes);
    let mouth = accessory
        .and_then(|a| a.mouth.clone())
        .or_else(|| base.and_then(|r| r.mouth.clone()))
        .or_else(|| neutral.and_then(|r| r.mouth.clone()));
    let overlay = accessory.and_then(|a| a.overlay.clone().or_else(|| a.animation.clone()));
    let full = base.and_then(|r| r.animation.clone());
    Face {
        eyes,
        mouth,
        overlay,
        full,
    }
}
