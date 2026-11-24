use super::theme::{
    AccentPreset, BackgroundMode, CornerMode, Rgb, ThemeSpec, migrate_legacy_theme,
    normalize_hex_color, parse_hex_color, resolve_palette, resolve_radii,
};

fn spec(bg_mode: BackgroundMode, accent: AccentPreset) -> ThemeSpec<'static> {
    ThemeSpec {
        bg_mode,
        custom_bg: None,
        custom_bg_dark: false,
        accent,
        last_preset_accent: AccentPreset::Sakura,
        custom_accent: None,
        bg_opacity: 1.0,
        card_opacity: 1.0,
    }
}

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
fn custom_accent_and_background_keep_contrast_and_fallback() {
    let mut s = spec(BackgroundMode::Light, AccentPreset::Custom);
    s.custom_accent = Some("#ffee55");
    let light = resolve_palette(s);
    assert_eq!(light.accent, Rgb::new(0xff, 0xee, 0x55));
    assert_eq!(light.text_on_accent, Rgb::new(0x1f, 0x19, 0x24));
    assert_eq!(light.text, Rgb::new(0x2d, 0x24, 0x31));

    s = spec(BackgroundMode::Dark, AccentPreset::Custom);
    s.custom_accent = Some("#2244aa");
    let dark = resolve_palette(s);
    assert_eq!(dark.text, Rgb::new(0xf2, 0xee, 0xf7));
    assert_eq!(dark.text_on_accent, Rgb::new(0xff, 0xff, 0xff));

    s = spec(BackgroundMode::Custom, AccentPreset::Mint);
    s.custom_bg = Some("#202430");
    let custom_dark = resolve_palette(s);
    assert_eq!(custom_dark.bg_solid, Rgb::new(0x20, 0x24, 0x30));
    assert_eq!(custom_dark.text, Rgb::new(0xf2, 0xee, 0xf7));

    let mut retained = spec(BackgroundMode::Custom, AccentPreset::Sky);
    retained.custom_bg_dark = true;
    retained.bg_opacity = 0.5;
    retained.card_opacity = 0.4;
    let ret = resolve_palette(retained);
    assert_eq!(ret.bg_solid, Rgb::new(0x14, 0x14, 0x17));
    assert_eq!(ret.bg.a, 128);
    assert_eq!(ret.surface.a, 102);

    let fallback = resolve_palette(ThemeSpec {
        bg_mode: BackgroundMode::Custom,
        custom_bg: Some("bad"),
        ..spec(BackgroundMode::Custom, AccentPreset::Sky)
    });
    assert_eq!(
        fallback,
        resolve_palette(spec(BackgroundMode::Light, AccentPreset::Sky))
    );
}

#[test]
fn corners_and_all_keys_round_trip() {
    for mode in BackgroundMode::ALL {
        assert_eq!(BackgroundMode::from_key(mode.key()), Some(mode));
    }
    for preset in AccentPreset::ALL {
        assert_eq!(AccentPreset::from_key(preset.key()), Some(preset));
    }
    for corner in CornerMode::ALL {
        assert_eq!(CornerMode::from_key(corner.key()), Some(corner));
    }
    assert_eq!(resolve_radii(CornerMode::Off, 14).card_px, 0);
    assert_eq!(resolve_radii(CornerMode::Off, 14).pill_px, 0);
    assert_eq!(resolve_radii(CornerMode::Round, 0).pill_px, 999);
    assert_eq!(resolve_radii(CornerMode::Custom, 99).card_px, 28);
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
