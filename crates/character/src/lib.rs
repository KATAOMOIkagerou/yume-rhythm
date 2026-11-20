//! L0 角色包、动画与 CPU 合成。见 docs/架构.md 第 7 节、docs/角色包格式.md。
//!
//! - `pack`：角色包格式、校验、加载（文件来源由调用方通过 `PackSource` 提供）。
//! - `anim`：动作模型（弹簧、点头、眨眼、音符）。
//! - `render`：分层 PNG 角色的渲染器 `LayeredCharacter`。
//! - `CharacterRenderer`：换角色的扩展点。以后支持 Live2D 等技术时，新建 crate 实现它即可。
#![forbid(unsafe_code)]

pub mod anim;
mod frame;
pub mod pack;
mod render;
pub mod sprite;

pub use frame::Frame;
pub use pack::{Pack, PackError, PackInfo, PackSource};
pub use render::LayeredCharacter;

use yume_beat::MotionFrame;
use yume_core::derive::PetScene;

/// 角色渲染器。桌宠线程每帧调用一次 `advance` 和 `render`。
pub trait CharacterRenderer: Send {
    /// 缩放 100%、DPI 100% 时的窗口尺寸（屏幕像素）。
    fn base_size(&self) -> (f32, f32);

    /// 推进动画。`now_ms` 是单调时钟，`motion` 是最新的节拍帧。
    fn advance(&mut self, now_ms: f64, scene: &PetScene, motion: &MotionFrame);

    /// 画到 `frame`。`scale` = 用户缩放 × 显示器 DPI 缩放。
    ///
    /// 返回 `false` 表示画面与上一帧看起来相同，`frame` 没有改动，调用方可以不提交。
    fn render(&mut self, frame: &mut Frame, scale: f32) -> bool;
}
