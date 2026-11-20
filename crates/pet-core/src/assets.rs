use crate::expressions::{
    is_safe_relative_asset_path, legacy_expressions, parse_expressions, ExpressionDefinition,
};
use crate::manifest::CharacterManifest;
use image::codecs::gif::GifDecoder;
use image::codecs::png::PngDecoder;
use image::codecs::webp::WebPDecoder;
use image::{AnimationDecoder, ImageFormat, RgbaImage};
use include_dir::{include_dir, Dir};
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

static EMBEDDED_CHARACTER_ASSETS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../assets/character");

/// A tightly cropped, premultiplied RGBA8 sprite storing only non-transparent pixels.
/// Dramatically reduces memory (75.7 MiB -> ~10 MiB) and skips empty pixels during `tiny-skia` blitting.
#[derive(Debug, Clone)]
pub struct TightSprite {
    pub offset_x: u32,
    pub offset_y: u32,
    pub width: u32,
    pub height: u32,
    pub canvas_width: u32,
    pub canvas_height: u32,
    /// Premultiplied RGBA8 pixels (`width * height * 4` bytes).
    pub premul_rgba: Vec<u8>,
}

impl TightSprite {
    /// Crops transparent margins around an `RgbaImage` and premultiplies RGB by Alpha.
    pub fn from_rgba_image(img: &RgbaImage) -> Self {
        let canvas_width = img.width();
        let canvas_height = img.height();
        let raw = img.as_raw();

        let mut min_x = canvas_width;
        let mut min_y = canvas_height;
        let mut max_x = 0u32;
        let mut max_y = 0u32;
        let mut found = false;

        for y in 0..canvas_height {
            let row_start = (y as usize) * (canvas_width as usize) * 4;
            for x in 0..canvas_width {
                let a = raw[row_start + (x as usize) * 4 + 3];
                if a != 0 {
                    if !found {
                        min_x = x;
                        max_x = x;
                        min_y = y;
                        max_y = y;
                        found = true;
                    } else {
                        if x < min_x {
                            min_x = x;
                        }
                        if x > max_x {
                            max_x = x;
                        }
                        if y < min_y {
                            min_y = y;
                        }
                        if y > max_y {
                            max_y = y;
                        }
                    }
                }
            }
        }

        if !found {
            return Self {
                offset_x: 0,
                offset_y: 0,
                width: 1,
                height: 1,
                canvas_width,
                canvas_height,
                premul_rgba: vec![0, 0, 0, 0],
            };
        }

        let width = max_x - min_x + 1;
        let height = max_y - min_y + 1;
        let mut premul_rgba = Vec::with_capacity((width as usize) * (height as usize) * 4);

        for y in min_y..=max_y {
            let row_start = (y as usize) * (canvas_width as usize) * 4;
            for x in min_x..=max_x {
                let idx = row_start + (x as usize) * 4;
                let r = raw[idx] as u16;
                let g = raw[idx + 1] as u16;
                let b = raw[idx + 2] as u16;
                let a = raw[idx + 3] as u16;
                if a == 255 {
                    premul_rgba.extend_from_slice(&[r as u8, g as u8, b as u8, 255]);
                } else if a == 0 {
                    premul_rgba.extend_from_slice(&[0, 0, 0, 0]);
                } else {
                    premul_rgba.push(((r * a + 127) / 255) as u8);
                    premul_rgba.push(((g * a + 127) / 255) as u8);
                    premul_rgba.push(((b * a + 127) / 255) as u8);
                    premul_rgba.push(a as u8);
                }
            }
        }

        Self {
            offset_x: min_x,
            offset_y: min_y,
            width,
            height,
            canvas_width,
            canvas_height,
            premul_rgba,
        }
    }

    pub fn byte_len(&self) -> usize {
        self.premul_rgba.len()
    }
}

#[derive(Debug, Clone)]
pub struct AnimationFrame {
    pub sprite: TightSprite,
    pub delay_ms: u32,
}

#[derive(Debug, Clone)]
pub struct AnimatedSprite {
    pub frames: Vec<AnimationFrame>,
    pub total_duration_ms: u64,
}

impl AnimatedSprite {
    pub fn from_single(sprite: TightSprite) -> Self {
        Self {
            frames: vec![AnimationFrame {
                sprite,
                delay_ms: 100,
            }],
            total_duration_ms: 100,
        }
    }

    pub fn frame_at(&self, elapsed_ms: u64) -> &TightSprite {
        if self.frames.len() <= 1 || self.total_duration_ms == 0 {
            return &self.frames[0].sprite;
        }
        let mut cursor = elapsed_ms % self.total_duration_ms;
        for frame in &self.frames {
            let d = u64::from(frame.delay_ms.max(16));
            if cursor < d {
                return &frame.sprite;
            }
            cursor -= d;
        }
        &self.frames[0].sprite
    }

    pub fn is_animated(&self) -> bool {
        self.frames.len() > 1
    }

    pub fn total_bytes(&self) -> usize {
        self.frames.iter().map(|f| f.sprite.byte_len()).sum()
    }
}

#[derive(Debug, Clone)]
pub struct LoadedCharacterAssets {
    pub manifest: CharacterManifest,
    pub expressions: Vec<ExpressionDefinition>,
    pub sprites: HashMap<String, AnimatedSprite>,
    pub source_hint: String,
    pub total_decoded_bytes: usize,
    pub full_canvas_equivalent_bytes: usize,
}

impl LoadedCharacterAssets {
    pub fn get_sprite(&self, filename: &str) -> Option<&AnimatedSprite> {
        self.sprites.get(filename)
    }

    /// Returns raw PNG bytes for the tray icon (`head_base.png`) from the embedded pack.
    pub fn embedded_tray_icon_rgba(size: u32) -> Result<(Vec<u8>, u32, u32), String> {
        let file = EMBEDDED_CHARACTER_ASSETS
            .get_file("head_base.png")
            .ok_or_else(|| "缺少内嵌托盘图标资源 head_base.png".to_string())?;
        let img = image::load_from_memory(file.contents())
            .map_err(|e| format!("解析托盘图标失败：{e}"))?;
        let resized = img
            .resize(size, size, image::imageops::FilterType::Triangle)
            .to_rgba8();
        Ok((resized.into_raw(), size, size))
    }
}

/// Resolves optional external character asset override directory (`YUME_ASSET_DIR` or `<exe_dir>/character_assets`).
pub fn resolve_external_asset_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("YUME_ASSET_DIR") {
        if !dir.is_empty() {
            let p = PathBuf::from(dir);
            if p.is_dir() {
                return Some(p);
            }
        }
    }
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))?;
    let candidate = exe_dir.join("character_assets");
    if candidate.is_dir() {
        Some(candidate)
    } else {
        None
    }
}

/// Loads character assets from external override directory if present, otherwise from embedded assets.
pub fn load_character_assets() -> Result<LoadedCharacterAssets, String> {
    if let Some(ext_dir) = resolve_external_asset_dir() {
        load_character_assets_from_dir(&ext_dir)
    } else {
        load_embedded_character_assets()
    }
}

pub fn load_embedded_character_assets() -> Result<LoadedCharacterAssets, String> {
    load_from_reader("内置默认素材包", |rel| {
        Ok(EMBEDDED_CHARACTER_ASSETS
            .get_file(rel)
            .map(|f| f.contents().to_vec()))
    })
}

pub fn load_character_assets_from_dir(root: &Path) -> Result<LoadedCharacterAssets, String> {
    let canonical_root = root
        .canonicalize()
        .map_err(|e| format!("无法访问外置素材目录：{e}"))?;
    let hint = canonical_root.display().to_string();
    load_from_reader(&hint, |rel| {
        if !is_safe_relative_asset_path(rel) {
            return Err(format!("非法素材路径：{rel}"));
        }
        let target = canonical_root.join(rel);
        if !target.exists() {
            return Ok(None);
        }
        let canonical_target = target
            .canonicalize()
            .map_err(|e| format!("无法解析素材路径 {rel}：{e}"))?;
        if !canonical_target.starts_with(&canonical_root) || !canonical_target.is_file() {
            return Err(format!("素材路径越界：{rel}"));
        }
        let bytes = fs::read(&canonical_target)
            .map_err(|e| format!("读取素材文件 {rel} 失败：{e}"))?;
        Ok(Some(bytes))
    })
}

fn load_from_reader<F>(source_hint: &str, read_file: F) -> Result<LoadedCharacterAssets, String>
where
    F: Fn(&str) -> Result<Option<Vec<u8>>, String>,
{
    let manifest_bytes = read_file("manifest.json")?
        .ok_or_else(|| "无法读取角色清单 manifest.json".to_string())?;
    let manifest_str =
        String::from_utf8(manifest_bytes).map_err(|_| "角色清单文件不是有效的 UTF-8".to_string())?;
    let manifest = CharacterManifest::parse_json(&manifest_str)?;

    let expressions = match read_file("expressions.json")? {
        Some(expr_bytes) => {
            let expr_str = String::from_utf8(expr_bytes)
                .map_err(|_| "表情配置文件不是有效的 UTF-8".to_string())?;
            parse_expressions(&expr_str)?
        }
        None => legacy_expressions(&manifest),
    };

    let mut required_files = BTreeSet::new();
    for f in manifest.collect_required_files() {
        required_files.insert(f);
    }
    for expr in &expressions {
        for f in expr.resources.file_paths() {
            required_files.insert(f.to_string());
        }
    }

    let mut sprites = HashMap::with_capacity(required_files.len());
    let mut total_decoded_bytes = 0usize;
    let mut full_canvas_equivalent_bytes = 0usize;

    for file in required_files {
        let bytes = read_file(&file)?
            .ok_or_else(|| format!("加载人物图层失败：{file}（文件不存在）"))?;
        let animated = decode_animated_tight_sprite(&bytes, &file)?;
        total_decoded_bytes += animated.total_bytes();
        for frame in &animated.frames {
            full_canvas_equivalent_bytes += (frame.sprite.canvas_width as usize)
                * (frame.sprite.canvas_height as usize)
                * 4;
        }
        sprites.insert(file, animated);
    }

    Ok(LoadedCharacterAssets {
        manifest,
        expressions,
        sprites,
        source_hint: source_hint.to_string(),
        total_decoded_bytes,
        full_canvas_equivalent_bytes,
    })
}

fn frame_delay_ms(delay: image::Delay) -> u32 {
    let (numer, denom) = delay.numer_denom_ms();
    let ms = if denom == 0 {
        100
    } else {
        (numer / denom).max(16)
    };
    ms
}

fn decode_animated_tight_sprite(bytes: &[u8], filename: &str) -> Result<AnimatedSprite, String> {
    let format = image::guess_format(bytes).ok();

    match format {
        Some(ImageFormat::Gif) => {
            let decoder = GifDecoder::new(Cursor::new(bytes))
                .map_err(|e| format!("解析 GIF 图层失败 {filename}：{e}"))?;
            let raw_frames = decoder
                .into_frames()
                .collect_frames()
                .map_err(|e| format!("解码 GIF 帧失败 {filename}：{e}"))?;
            if !raw_frames.is_empty() {
                let mut frames = Vec::with_capacity(raw_frames.len());
                let mut total = 0u64;
                for f in raw_frames {
                    let delay_ms = frame_delay_ms(f.delay());
                    let sprite = TightSprite::from_rgba_image(&f.into_buffer());
                    total += u64::from(delay_ms);
                    frames.push(AnimationFrame { sprite, delay_ms });
                }
                return Ok(AnimatedSprite {
                    frames,
                    total_duration_ms: total.max(16),
                });
            }
        }
        Some(ImageFormat::WebP) => {
            if let Ok(decoder) = WebPDecoder::new(Cursor::new(bytes)) {
                if decoder.has_animation() {
                    let raw_frames = decoder
                        .into_frames()
                        .collect_frames()
                        .map_err(|e| format!("解码动画 WebP 失败 {filename}：{e}"))?;
                    if !raw_frames.is_empty() {
                        let mut frames = Vec::with_capacity(raw_frames.len());
                        let mut total = 0u64;
                        for f in raw_frames {
                            let delay_ms = frame_delay_ms(f.delay());
                            let sprite = TightSprite::from_rgba_image(&f.into_buffer());
                            total += u64::from(delay_ms);
                            frames.push(AnimationFrame { sprite, delay_ms });
                        }
                        return Ok(AnimatedSprite {
                            frames,
                            total_duration_ms: total.max(16),
                        });
                    }
                }
            }
        }
        Some(ImageFormat::Png) => {
            if let Ok(decoder) = PngDecoder::new(Cursor::new(bytes)) {
                if decoder.is_apng().unwrap_or(false) {
                    if let Ok(apng) = decoder.apng() {
                        let raw_frames = apng
                            .into_frames()
                            .collect_frames()
                            .map_err(|e| format!("解码 APNG 帧失败 {filename}：{e}"))?;
                        if raw_frames.len() > 1 {
                            let mut frames = Vec::with_capacity(raw_frames.len());
                            let mut total = 0u64;
                            for f in raw_frames {
                                let delay_ms = frame_delay_ms(f.delay());
                                let sprite = TightSprite::from_rgba_image(&f.into_buffer());
                                total += u64::from(delay_ms);
                                frames.push(AnimationFrame { sprite, delay_ms });
                            }
                            return Ok(AnimatedSprite {
                                frames,
                                total_duration_ms: total.max(16),
                            });
                        }
                    }
                }
            }
        }
        _ => {}
    }

    let img = image::load_from_memory(bytes)
        .map_err(|e| format!("加载人物图层失败 {filename}：{e}"))?
        .to_rgba8();
    Ok(AnimatedSprite::from_single(TightSprite::from_rgba_image(
        &img,
    )))
}
