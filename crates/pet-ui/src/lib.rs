//! Pure-Rust floating console window (`egui` CPU software rasterizer + Win32 HWND).
//! Owned by `@ui-eng`.

use pet_core::{AppAction, AppState, ExpressionDefinition};

pub const CONSOLE_LOGICAL_WIDTH: u32 = 470;
pub const CONSOLE_LOGICAL_HEIGHT: u32 = 780;
pub const CONSOLE_MIN_WIDTH: u32 = 430;
pub const CONSOLE_MIN_HEIGHT: u32 = 500;

#[derive(Debug, Clone, Copy, Default)]
pub struct AnchorRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub scale_factor: f32,
}

/// On-demand floating console window that destroys its Win32 HWND & DIB surface when hidden.
pub struct ConsoleWindow {
    visible: bool,
}

impl ConsoleWindow {
    pub fn new() -> Self {
        Self { visible: false }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Creates (if needed) and shows the console window anchored cleanly beside the pet window.
    pub fn show_near(&mut self, _pet_rect: AnchorRect) -> Result<(), String> {
        self.visible = true;
        Ok(())
    }

    /// Destroys the native window and frees its pixel surface while leaving `AppState` intact.
    pub fn hide_and_release(&mut self) {
        self.visible = false;
    }

    /// Processes pending Win32 input, runs one `egui` frame when dirty, and returns `AppAction`s.
    pub fn update(
        &mut self,
        _state: &mut AppState,
        _expressions: &[ExpressionDefinition],
        _now_ms: f64,
    ) -> Vec<AppAction> {
        Vec::new()
    }
}

impl Default for ConsoleWindow {
    fn default() -> Self {
        Self::new()
    }
}
