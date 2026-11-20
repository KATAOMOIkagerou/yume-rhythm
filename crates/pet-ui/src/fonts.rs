use egui::{FontData, FontDefinitions, FontFamily};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Name of the primary Chinese UI font registered inside `egui::FontDefinitions`.
pub const CHINESE_FONT_NAME: &str = "zh_sans";

/// Candidate Windows Chinese font filenames in visual quality & coverage priority order.
const FONT_CANDIDATES: &[&str] = &[
    "msyh.ttc",
    "msyhbd.ttc",
    "msyh.ttf",
    "simhei.ttf",
    "Deng.ttf",
    "simsun.ttc",
];

#[derive(Debug, Clone)]
pub struct ChineseFontInfo {
    pub path: Option<PathBuf>,
    pub byte_len: usize,
}

/// Returns candidate Windows font directories (`%SystemRoot%\Fonts`, `C:\Windows\Fonts`).
fn windows_font_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::with_capacity(2);
    if let Some(root) = std::env::var_os("SystemRoot").or_else(|| std::env::var_os("WINDIR")) {
        let p = PathBuf::from(root).join("Fonts");
        if p.is_dir() {
            dirs.push(p);
        }
    }
    let fallback = PathBuf::from(r"C:\Windows\Fonts");
    if fallback.is_dir() && !dirs.iter().any(|d| d.eq_ignore_ascii_case_path(&fallback)) {
        dirs.push(fallback);
    }
    dirs
}

trait PathEqIgnoreCase {
    fn eq_ignore_ascii_case_path(&self, other: &Path) -> bool;
}

impl PathEqIgnoreCase for PathBuf {
    fn eq_ignore_ascii_case_path(&self, other: &Path) -> bool {
        self.to_string_lossy()
            .eq_ignore_ascii_case(&other.to_string_lossy())
    }
}

/// Locates the best available Windows Chinese system font file on disk.
pub fn find_chinese_font_path() -> Option<PathBuf> {
    for dir in windows_font_dirs() {
        for name in FONT_CANDIDATES {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// Loads the system Chinese font into `egui::FontDefinitions` (owned per `egui::Context`
/// so dropping the context when the console closes immediately frees font memory).
pub fn build_font_definitions() -> (FontDefinitions, ChineseFontInfo) {
    let mut defs = FontDefinitions::default();

    if let Some(path) = find_chinese_font_path() {
        if let Ok(bytes) = std::fs::read(&path) {
            let byte_len = bytes.len();
            let mut font_data = FontData::from_owned(bytes);
            font_data.index = 0;

            defs.font_data
                .insert(CHINESE_FONT_NAME.to_owned(), Arc::new(font_data));

            // Put Microsoft YaHei / SimHei first for crisp Chinese + Latin UI text,
            // retaining default fonts as fallback for emojis & special symbols.
            if let Some(prop) = defs.families.get_mut(&FontFamily::Proportional) {
                prop.insert(0, CHINESE_FONT_NAME.to_owned());
            }
            // Keep monospace digits crisp while falling back to Chinese font for any CJK chars.
            if let Some(mono) = defs.families.get_mut(&FontFamily::Monospace) {
                mono.push(CHINESE_FONT_NAME.to_owned());
            }

            return (
                defs,
                ChineseFontInfo {
                    path: Some(path),
                    byte_len,
                },
            );
        }
    }

    (
        defs,
        ChineseFontInfo {
            path: None,
            byte_len: 0,
        },
    )
}
