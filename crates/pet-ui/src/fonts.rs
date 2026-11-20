use egui::{FontData, FontDefinitions, FontFamily};
use std::ffi::c_void;
use std::fs::File;
use std::os::windows::io::AsRawHandle;
use std::path::{Path, PathBuf};
use std::ptr::null;
use std::sync::{Arc, OnceLock};
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::System::Memory::{MapViewOfFile, FILE_MAP_READ, PAGE_READONLY};

#[link(name = "kernel32")]
extern "system" {
    fn CreateFileMappingW(
        hfile: HANDLE,
        lpfilemappingattributes: *const c_void,
        flprotect: u32,
        dwmaximumsizehigh: u32,
        dwmaximumsizelow: u32,
        lpname: *const u16,
    ) -> HANDLE;
}

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

struct MappedChineseFont {
    path: PathBuf,
    bytes: &'static [u8],
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

/// Maps the system Chinese font file read-only (`FILE_MAP_READ`) once for the lifetime of the
/// process so `egui` / `ab_glyph` consume a zero-copy `Cow::Borrowed(&'static [u8])` slice
/// backed by Windows' shared file-mapping pages (**0 MiB private heap allocation**).
fn mapped_chinese_font() -> Option<&'static MappedChineseFont> {
    static MAPPED: OnceLock<Option<MappedChineseFont>> = OnceLock::new();
    MAPPED
        .get_or_init(|| {
            let path = find_chinese_font_path()?;
            let file = File::open(&path).ok()?;
            let meta = file.metadata().ok()?;
            let len = usize::try_from(meta.len()).ok()?;
            if len == 0 {
                return None;
            }

            let hfile: HANDLE = file.as_raw_handle();
            unsafe {
                let mapping =
                    CreateFileMappingW(hfile, null(), PAGE_READONLY, 0, 0, null());
                if mapping.is_null() {
                    return None;
                }
                let view = MapViewOfFile(mapping, FILE_MAP_READ, 0, 0, len);
                // Windows keeps the mapped view valid after closing both mapping and file handles.
                CloseHandle(mapping);
                drop(file);

                if view.Value.is_null() {
                    return None;
                }
                let bytes = std::slice::from_raw_parts(view.Value as *const u8, len);
                Some(MappedChineseFont { path, bytes })
            }
        })
        .as_ref()
}

/// Registers the read-only memory-mapped system Chinese font into `egui::FontDefinitions`
/// using `FontData::from_static` (`Cow::Borrowed` — zero private heap copies).
pub fn build_font_definitions() -> (FontDefinitions, ChineseFontInfo) {
    let mut defs = FontDefinitions::default();

    if let Some(mapped) = mapped_chinese_font() {
        let mut font_data = FontData::from_static(mapped.bytes);
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
                path: Some(mapped.path.clone()),
                byte_len: mapped.bytes.len(),
            },
        );
    }

    (
        defs,
        ChineseFontInfo {
            path: None,
            byte_len: 0,
        },
    )
}
