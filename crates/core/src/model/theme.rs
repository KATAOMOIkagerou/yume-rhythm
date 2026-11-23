//! 控制台外观模型：背景模式 + 强调色拆分，以及自定义颜色对比度推导（功能清单第 2 节）。
//!
//! 所有语义色都在这里确定性派生，避免界面层硬编码色值或出现低对比度文字。

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackgroundMode {
    #[default]
    Light,
    Dark,
    Custom,
}

impl BackgroundMode {
    pub const ALL: [Self; 3] = [Self::Light, Self::Dark, Self::Custom];

    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
            Self::Custom => "custom",
        }
    }

    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.key() == key)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccentPreset {
    #[default]
    Sakura,
    Sky,
    Lavender,
    Sunset,
    Mint,
}

impl AccentPreset {
    pub const ALL: [Self; 5] = [
        Self::Sakura,
        Self::Sky,
        Self::Lavender,
        Self::Sunset,
        Self::Mint,
    ];

    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Sakura => "sakura",
            Self::Sky => "sky",
            Self::Lavender => "lavender",
            Self::Sunset => "sunset",
            Self::Mint => "mint",
        }
    }

    #[must_use]
    pub const fn color(self) -> Rgb {
        match self {
            Self::Sakura => Rgb::new(0xd6, 0x55, 0x8c),
            Self::Sky => Rgb::new(0x2d, 0x7f, 0xd6),
            Self::Lavender => Rgb::new(0x7c, 0x5c, 0xde),
            Self::Sunset => Rgb::new(0xe0, 0x70, 0x3a),
            Self::Mint => Rgb::new(0x26, 0x99, 0x74),
        }
    }

    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|preset| preset.key() == key)
    }
}

/// 将旧版 `theme`（v0.2 预设名）迁移为「背景模式 + 强调色」。
#[must_use]
pub fn migrate_legacy_theme(key: &str) -> (BackgroundMode, AccentPreset) {
    match key {
        "night" => (BackgroundMode::Dark, AccentPreset::Sakura),
        "sky" => (BackgroundMode::Light, AccentPreset::Sky),
        "lavender" => (BackgroundMode::Light, AccentPreset::Lavender),
        "peach" => (BackgroundMode::Light, AccentPreset::Sunset),
        "mint" => (BackgroundMode::Light, AccentPreset::Mint),
        _ => (BackgroundMode::Light, AccentPreset::Sakura),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    #[must_use]
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    #[must_use]
    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    #[must_use]
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
}

/// 控制台界面全部语义色。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThemePalette {
    pub bg: Rgb,
    pub surface: Rgb,
    pub surface_alt: Rgb,
    pub surface_hover: Rgb,
    pub border: Rgb,
    pub divider: Rgb,
    pub text: Rgb,
    pub text_secondary: Rgb,
    pub text_muted: Rgb,
    pub text_on_accent: Rgb,
    pub accent: Rgb,
    pub accent_hover: Rgb,
    pub accent_pressed: Rgb,
    pub accent_soft: Rgb,
    pub accent_strong_text: Rgb,
    pub violet: Rgb,
    pub violet_soft: Rgb,
    pub green: Rgb,
    pub green_soft: Rgb,
    pub amber: Rgb,
    pub amber_soft: Rgb,
    pub red: Rgb,
    pub red_soft: Rgb,
    pub scrim: Rgba,
}

/// 解析 `#RRGGBB` 或 `RRGGBB` 十六进制颜色并规范化为 `#rrggbb`。
#[must_use]
pub fn parse_hex_color(input: &str) -> Option<Rgb> {
    let trimmed = input.trim();
    let hex = trimmed.strip_prefix('#').unwrap_or(trimmed);
    if hex.len() != 6 || !hex.is_ascii() {
        return None;
    }
    let r = u8::from_str_radix(hex.get(0..2)?, 16).ok()?;
    let g = u8::from_str_radix(hex.get(2..4)?, 16).ok()?;
    let b = u8::from_str_radix(hex.get(4..6)?, 16).ok()?;
    Some(Rgb::new(r, g, b))
}

#[must_use]
pub fn normalize_hex_color(input: &str) -> Option<String> {
    parse_hex_color(input).map(Rgb::to_hex)
}

/// 依据背景模式、自定义背景色、强调色预设和可选自定义强调色计算完整调色板。
#[must_use]
pub fn resolve_palette(
    bg_mode: BackgroundMode,
    custom_bg: Option<&str>,
    accent: AccentPreset,
    custom_accent: Option<&str>,
) -> ThemePalette {
    let (bg, surface, dark) = resolve_base(bg_mode, custom_bg);
    let accent_rgb = custom_accent
        .and_then(parse_hex_color)
        .unwrap_or_else(|| accent.color());
    let mut palette = build_palette(bg, surface, accent_rgb, dark);
    apply_accent(&mut palette, accent_rgb, dark);
    palette
}

fn resolve_base(mode: BackgroundMode, custom_bg: Option<&str>) -> (Rgb, Rgb, bool) {
    const LIGHT_BG: Rgb = Rgb::new(0xf7, 0xf7, 0xf9);
    const LIGHT_SURFACE: Rgb = Rgb::new(0xff, 0xff, 0xff);
    match mode {
        BackgroundMode::Light => (LIGHT_BG, LIGHT_SURFACE, false),
        BackgroundMode::Dark => (Rgb::new(0x14, 0x14, 0x17), Rgb::new(0x1e, 0x1e, 0x23), true),
        BackgroundMode::Custom => {
            if let Some(c) = custom_bg.and_then(parse_hex_color) {
                let dark = perceived_luma(c) < 128;
                let surface = if dark {
                    mix(c, Rgb::new(0xff, 0xff, 0xff), 8)
                } else {
                    mix(c, Rgb::new(0xff, 0xff, 0xff), 70)
                };
                (c, surface, dark)
            } else {
                (LIGHT_BG, LIGHT_SURFACE, false)
            }
        }
    }
}

fn build_palette(bg: Rgb, surface: Rgb, accent: Rgb, dark: bool) -> ThemePalette {
    if dark {
        let surface_alt = mix(surface, accent, 10);
        let surface_hover = mix(surface, accent, 14);
        let border = mix(mix(surface, Rgb::new(0xff, 0xff, 0xff), 14), accent, 10);
        let divider = mix(mix(surface, Rgb::new(0xff, 0xff, 0xff), 8), accent, 8);
        ThemePalette {
            bg,
            surface,
            surface_alt,
            surface_hover,
            border,
            divider,
            text: Rgb::new(0xf2, 0xee, 0xf7),
            text_secondary: Rgb::new(0xb8, 0xaf, 0xc7),
            text_muted: Rgb::new(0x88, 0x7e, 0x99),
            text_on_accent: Rgb::new(0xff, 0xff, 0xff),
            accent,
            accent_hover: accent,
            accent_pressed: accent,
            accent_soft: surface_alt,
            accent_strong_text: accent,
            violet: Rgb::new(0xa4, 0x8d, 0xf5),
            violet_soft: Rgb::new(0x2e, 0x27, 0x47),
            green: Rgb::new(0x4a, 0xc2, 0x8c),
            green_soft: Rgb::new(0x1f, 0x36, 0x2c),
            amber: Rgb::new(0xe5, 0xaa, 0x42),
            amber_soft: Rgb::new(0x3a, 0x30, 0x1c),
            red: Rgb::new(0xe8, 0x6b, 0x6b),
            red_soft: Rgb::new(0x3d, 0x23, 0x28),
            scrim: Rgba::new(0x0b, 0x0a, 0x10, 0x99),
        }
    } else {
        let surface_alt = mix(bg, accent, 6);
        let surface_hover = mix(bg, accent, 10);
        let border = mix(mix(bg, Rgb::new(0x00, 0x00, 0x00), 8), accent, 8);
        let divider = mix(mix(bg, Rgb::new(0x00, 0x00, 0x00), 4), accent, 6);
        ThemePalette {
            bg,
            surface,
            surface_alt,
            surface_hover,
            border,
            divider,
            text: Rgb::new(0x2d, 0x24, 0x31),
            text_secondary: Rgb::new(0x75, 0x68, 0x7b),
            text_muted: Rgb::new(0xa9, 0x9f, 0xae),
            text_on_accent: Rgb::new(0xff, 0xff, 0xff),
            accent,
            accent_hover: accent,
            accent_pressed: accent,
            accent_soft: surface_alt,
            accent_strong_text: accent,
            violet: Rgb::new(0x84, 0x67, 0xe8),
            violet_soft: Rgb::new(0xef, 0xea, 0xfd),
            green: Rgb::new(0x2c, 0x9a, 0x69),
            green_soft: Rgb::new(0xe2, 0xf4, 0xeb),
            amber: Rgb::new(0xb9, 0x7a, 0x0e),
            amber_soft: Rgb::new(0xfc, 0xf2, 0xda),
            red: Rgb::new(0xc9, 0x43, 0x43),
            red_soft: Rgb::new(0xfc, 0xe9, 0xe9),
            scrim: Rgba::new(0x2d, 0x24, 0x31, 0x66),
        }
    }
}

fn apply_accent(palette: &mut ThemePalette, accent: Rgb, dark: bool) {
    palette.accent = accent;
    palette.text_on_accent = if perceived_luma(accent) > 165 {
        Rgb::new(0x1f, 0x19, 0x24)
    } else {
        Rgb::new(0xff, 0xff, 0xff)
    };
    if dark {
        palette.accent_hover = mix(accent, Rgb::new(0xff, 0xff, 0xff), 14);
        palette.accent_pressed = mix(accent, Rgb::new(0x00, 0x00, 0x00), 16);
        palette.accent_soft = mix(accent, palette.surface, 76);
        palette.accent_strong_text = mix(accent, Rgb::new(0xff, 0xff, 0xff), 42);
    } else {
        palette.accent_hover = mix(accent, Rgb::new(0x00, 0x00, 0x00), 10);
        palette.accent_pressed = mix(accent, Rgb::new(0x00, 0x00, 0x00), 20);
        palette.accent_soft = mix(accent, Rgb::new(0xff, 0xff, 0xff), 86);
        palette.accent_strong_text = mix(accent, Rgb::new(0x00, 0x00, 0x00), 22);
    }
}

fn perceived_luma(c: Rgb) -> u32 {
    (u32::from(c.r) * 299 + u32::from(c.g) * 587 + u32::from(c.b) * 114) / 1000
}

fn mix(a: Rgb, b: Rgb, b_pct: u16) -> Rgb {
    let a_pct = 100 - b_pct;
    let blend = |x: u8, y: u8| -> u8 {
        let v = (u16::from(x) * a_pct + u16::from(y) * b_pct + 50) / 100;
        u8::try_from(v.min(255)).unwrap_or(255)
    };
    Rgb::new(blend(a.r, b.r), blend(a.g, b.g), blend(a.b, b.b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_parsing_accepts_hash_or_bare() {
        assert_eq!(parse_hex_color("#3366CC"), Some(Rgb::new(0x33, 0x66, 0xcc)));
        assert_eq!(
            parse_hex_color(" ff8800 "),
            Some(Rgb::new(0xff, 0x88, 0x00))
        );
        assert_eq!(normalize_hex_color("AbCdEf").as_deref(), Some("#abcdef"));
        assert_eq!(parse_hex_color("#12345"), None);
        assert_eq!(parse_hex_color("#gggggg"), None);
    }

    #[test]
    fn custom_accent_and_background_keep_contrast() {
        let light = resolve_palette(
            BackgroundMode::Light,
            None,
            AccentPreset::Sakura,
            Some("#ffee55"),
        );
        assert_eq!(light.accent, Rgb::new(0xff, 0xee, 0x55));
        assert_eq!(light.text_on_accent, Rgb::new(0x1f, 0x19, 0x24));
        assert_eq!(light.text, Rgb::new(0x2d, 0x24, 0x31));

        let dark = resolve_palette(
            BackgroundMode::Dark,
            None,
            AccentPreset::Sky,
            Some("#2244aa"),
        );
        assert_eq!(dark.text, Rgb::new(0xf2, 0xee, 0xf7));
        assert_eq!(dark.text_on_accent, Rgb::new(0xff, 0xff, 0xff));

        let custom_dark = resolve_palette(
            BackgroundMode::Custom,
            Some("#202430"),
            AccentPreset::Mint,
            None,
        );
        assert_eq!(custom_dark.bg, Rgb::new(0x20, 0x24, 0x30));
        assert_eq!(custom_dark.text, Rgb::new(0xf2, 0xee, 0xf7));

        let custom_light = resolve_palette(
            BackgroundMode::Custom,
            Some("#e8f0fe"),
            AccentPreset::Lavender,
            None,
        );
        assert_eq!(custom_light.bg, Rgb::new(0xe8, 0xf0, 0xfe));
        assert_eq!(custom_light.text, Rgb::new(0x2d, 0x24, 0x31));

        let fallback =
            resolve_palette(BackgroundMode::Custom, Some("bad"), AccentPreset::Sky, None);
        assert_eq!(
            fallback,
            resolve_palette(BackgroundMode::Light, None, AccentPreset::Sky, None)
        );
    }

    #[test]
    fn all_keys_round_trip_and_legacy_migrates() {
        for mode in BackgroundMode::ALL {
            assert_eq!(BackgroundMode::from_key(mode.key()), Some(mode));
        }
        for preset in AccentPreset::ALL {
            assert_eq!(AccentPreset::from_key(preset.key()), Some(preset));
        }
        assert_eq!(
            migrate_legacy_theme("night"),
            (BackgroundMode::Dark, AccentPreset::Sakura)
        );
        assert_eq!(
            migrate_legacy_theme("peach"),
            (BackgroundMode::Light, AccentPreset::Sunset)
        );
        assert_eq!(
            migrate_legacy_theme("sky"),
            (BackgroundMode::Light, AccentPreset::Sky)
        );
    }
}
