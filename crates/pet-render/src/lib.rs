//! Pure-Rust Win32 `WS_EX_LAYERED` + `UpdateLayeredWindow` pet window & `tiny-skia` tight-bbox compositor.
//! Owned by `@render-eng`.

use pet_core::{
    ActivityMode, AnimationPose, LoadedCharacterAssets, ResolvedFaceLayers,
};
use std::path::PathBuf;

pub const PET_LOGICAL_WIDTH: u32 = 540;
pub const PET_LOGICAL_HEIGHT: u32 = 590;

#[derive(Debug, Clone, Copy, Default)]
pub struct WindowRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub scale_factor: f32,
}

#[derive(Debug, Clone)]
pub struct PetRenderInput<'a> {
    pub assets: &'a LoadedCharacterAssets,
    pub pose: &'a AnimationPose,
    pub layers: &'a ResolvedFaceLayers,
    pub activity_mode: ActivityMode,
    pub music_running: bool,
    pub timer_active: bool,
    pub pet_locked: bool,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone)]
pub enum PetWindowEvent {
    OpenConsoleClicked,
    ToggleLockClicked,
    DroppedAudioFile(PathBuf),
    KeySpace,
    KeyQ,
    KeyH,
    CloseRequested,
}

/// CPU `tiny-skia` tight-bbox compositor producing premultiplied RGBA8 / BGRA8 frames.
pub struct PetCompositor {
    width: u32,
    height: u32,
    bgra_buffer: Vec<u8>,
}

impl PetCompositor {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            bgra_buffer: vec![0u8; (width as usize) * (height as usize) * 4],
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if self.width != width || self.height != height {
            self.width = width;
            self.height = height;
            self.bgra_buffer
                .resize((width as usize) * (height as usize) * 4, 0);
        }
    }

    pub fn invalidate_static_cache(&mut self) {}

    /// Renders a single pet frame into an internal premultiplied BGRA buffer ready for `UpdateLayeredWindow`.
    pub fn render_frame(&mut self, _input: &PetRenderInput<'_>) -> &[u8] {
        &self.bgra_buffer
    }

    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

/// Native Win32 transparent, borderless, always-on-top layered pet window.
pub struct PetWindow {
    locked: bool,
}

impl PetWindow {
    pub fn create() -> Result<Self, String> {
        Ok(Self { locked: false })
    }

    pub fn set_locked(&mut self, locked: bool) -> Result<(), String> {
        self.locked = locked;
        Ok(())
    }

    pub fn is_locked(&self) -> bool {
        self.locked
    }

    pub fn present(&mut self, _input: &PetRenderInput<'_>) -> Result<(), String> {
        Ok(())
    }

    pub fn poll_events(&mut self) -> Vec<PetWindowEvent> {
        Vec::new()
    }

    pub fn window_rect(&self) -> WindowRect {
        WindowRect {
            x: 120,
            y: 120,
            width: PET_LOGICAL_WIDTH as i32,
            height: PET_LOGICAL_HEIGHT as i32,
            scale_factor: 1.0,
        }
    }
}
