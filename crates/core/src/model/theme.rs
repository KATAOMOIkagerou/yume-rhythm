//! 控制台主题配色与自定义强调色（功能清单第 2 节）。
//!
//! 所有语义色都在这里确定性派生，避免界面层硬编码色值或出现低对比度文字。

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemePreset {
    #[default]
    Sakura,
    Sky,
    Lavender,
    Mint,
    Peach,
    Night,
}

impl ThemePreset {
    pub const ALL: [Self; 6] = [
        Self::Sakura,
        Self::Sky,
        Self::Lavender,
        Self::Mint,
        Self::Peach,
        Self::Night,
    ];

    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Sakura => "sakura",
            Self::Sky => "sky",
            Self::Lavender => "lavender",
            Self::Mint => "mint",
            Self::Peach => "peach",
            Self::Night => "night",
        }
    }

    #[must_use]
    pub const fn is_dark(self) -> bool {
        matches!(self, Self::Night)
    }

    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|preset| preset.key() == key)
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

/// 依据预设和可选自定义强调色计算完整调色板。
#[must_use]
pub fn resolve_palette(preset: ThemePreset, custom_accent: Option<&str>) -> ThemePalette {
    let mut palette = preset_palette(preset);
    if let Some(accent) = custom_accent.and_then(parse_hex_color) {
        apply_accent(&mut palette, accent, preset.is_dark());
    }
    palette
}

fn preset_palette(preset: ThemePreset) -> ThemePalette {
    match preset {
        ThemePreset::Sakura => light_palette(
            Rgb::new(0xf8, 0xf4, 0xf7),
            Rgb::new(0xf5, 0xee, 0xf3),
            Rgb::new(0xf1, 0xe8, 0xee),
            Rgb::new(0xec, 0xe1, 0xe8),
            Rgb::new(0xf2, 0xea, 0xef),
            Rgb::new(0xd6, 0x55, 0x8c),
            Rgb::new(0xc4, 0x46, 0x7c),
            Rgb::new(0xb0, 0x3b, 0x6e),
            Rgb::new(0xfc, 0xe8, 0xf1),
            Rgb::new(0xb3, 0x32, 0x6a),
        ),
        ThemePreset::Sky => light_palette(
            Rgb::new(0xf2, 0xf7, 0xfc),
            Rgb::new(0xea, 0xf2, 0xfa),
            Rgb::new(0xe2, 0xec, 0xf7),
            Rgb::new(0xd9, 0xe5, 0xf2),
            Rgb::new(0xe6, 0xef, 0xf8),
            Rgb::new(0x2d, 0x7f, 0xd6),
            Rgb::new(0x23, 0x6f, 0xc2),
            Rgb::new(0x1b, 0x5d, 0xa8),
            Rgb::new(0xe3, 0xf0, 0xfd),
            Rgb::new(0x1c, 0x5b, 0xa6),
        ),
        ThemePreset::Lavender => light_palette(
            Rgb::new(0xf6, 0xf4, 0xfc),
            Rgb::new(0xef, 0xeb, 0xf9),
            Rgb::new(0xe8, 0xe2, 0xf6),
            Rgb::new(0xe2, 0xdc, 0xf0),
            Rgb::new(0xec, 0xe7, 0xf6),
            Rgb::new(0x7c, 0x5c, 0xde),
            Rgb::new(0x6b, 0x4b, 0xcf),
            Rgb::new(0x5b, 0x3d, 0xba),
            Rgb::new(0xee, 0xe9, 0xfd),
            Rgb::new(0x59, 0x3b, 0xb5),
        ),
        ThemePreset::Mint => light_palette(
            Rgb::new(0xf2, 0xf9, 0xf6),
            Rgb::new(0xe9, 0xf4, 0xef),
            Rgb::new(0xe0, 0xef, 0xe8),
            Rgb::new(0xd6, 0xe8, 0xe0),
            Rgb::new(0xe4, 0xf1, 0xeb),
            Rgb::new(0x26, 0x99, 0x74),
            Rgb::new(0x1e, 0x86, 0x65),
            Rgb::new(0x17, 0x72, 0x55),
            Rgb::new(0xe0, 0xf5, 0xed),
            Rgb::new(0x17, 0x6e, 0x52),
        ),
        ThemePreset::Peach => light_palette(
            Rgb::new(0xfc, 0xf5, 0xf0),
            Rgb::new(0xf9, 0xee, 0xe6),
            Rgb::new(0xf5, 0xe6, 0xdc),
            Rgb::new(0xef, 0xde, 0xd2),
            Rgb::new(0xf6, 0xea, 0xe2),
            Rgb::new(0xe0, 0x6c, 0x43),
            Rgb::new(0xce, 0x5c, 0x34),
            Rgb::new(0xb8, 0x4c, 0x26),
            Rgb::new(0xfd, 0xeb, 0xe2),
            Rgb::new(0xb3, 0x47, 0x22),
        ),
        ThemePreset::Night => ThemePalette {
            bg: Rgb::new(0x18, 0x16, 0x21),
            surface: Rgb::new(0x22, 0x1f, 0x2d),
            surface_alt: Rgb::new(0x2c, 0x28, 0x3a),
            surface_hover: Rgb::new(0x35, 0x30, 0x45),
            border: Rgb::new(0x3b, 0x35, 0x4d),
            divider: Rgb::new(0x31, 0x2c, 0x40),
            text: Rgb::new(0xf2, 0xee, 0xf7),
            text_secondary: Rgb::new(0xb8, 0xaf, 0xc7),
            text_muted: Rgb::new(0x88, 0x7e, 0x99),
            text_on_accent: Rgb::new(0xff, 0xff, 0xff),
            accent: Rgb::new(0xe5, 0x6b, 0xa2),
            accent_hover: Rgb::new(0xee, 0x7e, 0xb0),
            accent_pressed: Rgb::new(0xd1, 0x57, 0x8e),
            accent_soft: Rgb::new(0x3c, 0x25, 0x38),
            accent_strong_text: Rgb::new(0xf9, 0xa8, 0xcc),
            violet: Rgb::new(0xa4, 0x8d, 0xf5),
            violet_soft: Rgb::new(0x2e, 0x27, 0x47),
            green: Rgb::new(0x4a, 0xc2, 0x8c),
            green_soft: Rgb::new(0x1f, 0x36, 0x2c),
            amber: Rgb::new(0xe5, 0xaa, 0x42),
            amber_soft: Rgb::new(0x3a, 0x30, 0x1c),
            red: Rgb::new(0xe8, 0x6b, 0x6b),
            red_soft: Rgb::new(0x3d, 0x23, 0x28),
            scrim: Rgba::new(0x0b, 0x0a, 0x10, 0x99),
        },
    }
}

#[allow(clippy::too_many_arguments, reason = "色板构造参数清晰对应语义字段")]
const fn light_palette(
    bg: Rgb,
    surface_alt: Rgb,
    surface_hover: Rgb,
    border: Rgb,
    divider: Rgb,
    accent: Rgb,
    accent_hover: Rgb,
    accent_pressed: Rgb,
    accent_soft: Rgb,
    accent_strong_text: Rgb,
) -> ThemePalette {
    ThemePalette {
        bg,
        surface: Rgb::new(0xff, 0xff, 0xff),
        surface_alt,
        surface_hover,
        border,
        divider,
        text: Rgb::new(0x2d, 0x24, 0x31),
        text_secondary: Rgb::new(0x75, 0x68, 0x7b),
        text_muted: Rgb::new(0xa9, 0x9f, 0xae),
        text_on_accent: Rgb::new(0xff, 0xff, 0xff),
        accent,
        accent_hover,
        accent_pressed,
        accent_soft,
        accent_strong_text,
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
    fn custom_accent_adjusts_text_contrast_on_bright_color() {
        let bright = resolve_palette(ThemePreset::Sakura, Some("#ffee55"));
        assert_eq!(bright.accent, Rgb::new(0xff, 0xee, 0x55));
        assert_eq!(bright.text_on_accent, Rgb::new(0x1f, 0x19, 0x24));

        let dark_btn = resolve_palette(ThemePreset::Night, Some("#2244aa"));
        assert_eq!(dark_btn.text_on_accent, Rgb::new(0xff, 0xff, 0xff));
    }

    #[test]
    fn all_presets_round_trip_keys() {
        for preset in ThemePreset::ALL {
            assert_eq!(ThemePreset::from_key(preset.key()), Some(preset));
        }
    }
}
