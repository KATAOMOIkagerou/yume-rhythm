//! v0.3.1 外观、圆角、透明度与中英文字体规则测试。

use crate::clock::{Ctx, SplitMix64, WallMs};
use crate::command::{Command, SettingsCommand};
use crate::model::settings::FONT_SIZE_DEFAULT;
use crate::model::theme::{
    AccentPreset, BackgroundMode, CORNER_RADIUS_DEFAULT, CornerMode, ThemeSpec, resolve_palette,
    resolve_radii,
};
use crate::state::AppState;

fn send(state: &mut AppState, cmd: Command) {
    let mut rng = SplitMix64::new(11);
    let mut ctx = Ctx::new(WallMs(1_000_000), &mut rng);
    super::update(state, cmd, &mut ctx);
}

fn current_palette(state: &AppState) -> crate::model::theme::ThemePalette {
    let s = state.settings();
    resolve_palette(ThemeSpec {
        bg_mode: s.background,
        custom_bg: s.custom_background.as_deref(),
        custom_bg_dark: s.custom_bg_dark,
        accent: s.accent,
        last_preset_accent: s.last_preset_accent,
        custom_accent: s.custom_accent.as_deref(),
        bg_opacity: s.bg_opacity,
        card_opacity: s.card_opacity,
    })
}

#[test]
fn dark_to_custom_retains_dark_base_until_hex_applied() {
    let mut state = AppState::default();
    send(
        &mut state,
        Command::Settings(SettingsCommand::SetBackgroundMode(BackgroundMode::Dark)),
    );
    send(
        &mut state,
        Command::Settings(SettingsCommand::SetBackgroundMode(BackgroundMode::Custom)),
    );
    assert!(state.settings().custom_bg_dark);
    let palette = current_palette(&state);
    assert_eq!(palette.bg_solid.r, 0x14);
    assert_eq!(palette.text.r, 0xf2);

    send(
        &mut state,
        Command::Settings(SettingsCommand::SetCustomBackground(Some("#f0f4ff".into()))),
    );
    assert!(!state.settings().custom_bg_dark);
    let light_custom = current_palette(&state);
    assert_eq!(light_custom.bg_solid.r, 0xf0);
    assert_eq!(light_custom.text.r, 0x2d);

    // 切回深色后再切自定义，仍保留黑色底等待新输入。
    send(
        &mut state,
        Command::Settings(SettingsCommand::SetBackgroundMode(BackgroundMode::Dark)),
    );
    send(
        &mut state,
        Command::Settings(SettingsCommand::SetBackgroundMode(BackgroundMode::Custom)),
    );
    assert_eq!(state.settings().custom_background, None);
    assert_eq!(current_palette(&state).bg_solid.r, 0x14);
}

#[test]
fn accent_custom_chip_and_fallback_color() {
    let mut state = AppState::default();
    send(
        &mut state,
        Command::Settings(SettingsCommand::SetAccent(AccentPreset::Lavender)),
    );
    send(
        &mut state,
        Command::Settings(SettingsCommand::SetAccent(AccentPreset::Custom)),
    );
    assert_eq!(state.settings().accent, AccentPreset::Custom);
    assert_eq!(state.settings().last_preset_accent, AccentPreset::Lavender);
    // 未输入前保留切换前的强调色（丁香紫）。
    assert_eq!(
        Some(current_palette(&state).accent),
        AccentPreset::Lavender.preset_color()
    );

    send(
        &mut state,
        Command::Settings(SettingsCommand::SetCustomAccent(Some("#1188cc".into()))),
    );
    assert_eq!(current_palette(&state).accent.to_hex(), "#1188cc");

    send(
        &mut state,
        Command::Settings(SettingsCommand::SetCustomAccent(None)),
    );
    assert_eq!(state.settings().accent, AccentPreset::Custom);
    assert_eq!(
        Some(current_palette(&state).accent),
        AccentPreset::Lavender.preset_color()
    );
}

#[test]
fn bg_card_opacity_corner_and_bilingual_fonts_update_and_clamp() {
    let mut state = AppState::default();
    send(
        &mut state,
        Command::Settings(SettingsCommand::SetBgOpacity(0.32)),
    );
    send(
        &mut state,
        Command::Settings(SettingsCommand::SetCardOpacity(-1.0)),
    );
    assert!((state.settings().bg_opacity - 0.3).abs() < 1e-6);
    assert!((state.settings().card_opacity - 0.0).abs() < 1e-6);

    send(
        &mut state,
        Command::Settings(SettingsCommand::SetCornerMode(CornerMode::Off)),
    );
    assert_eq!(
        resolve_radii(
            state.settings().corner_mode,
            state.settings().custom_corner_radius
        )
        .card_px,
        0
    );
    send(
        &mut state,
        Command::Settings(SettingsCommand::SetCornerMode(CornerMode::Custom)),
    );
    send(
        &mut state,
        Command::Settings(SettingsCommand::SetCustomCornerRadius(50)),
    );
    assert_eq!(state.settings().custom_corner_radius, 28);

    send(
        &mut state,
        Command::Settings(SettingsCommand::SetZhFontFamily(Some("KaiTi".into()))),
    );
    send(
        &mut state,
        Command::Settings(SettingsCommand::SetEnFontFamily(Some("Inter".into()))),
    );
    send(
        &mut state,
        Command::Settings(SettingsCommand::SetFontSize(22)),
    );
    assert_eq!(state.settings().font_size, 18);
    assert_eq!(state.settings().zh_font_family.as_deref(), Some("KaiTi"));
    assert_eq!(state.settings().en_font_family.as_deref(), Some("Inter"));

    send(
        &mut state,
        Command::Settings(SettingsCommand::CustomFontLoaded {
            path: "C:/fonts/en.ttf".into(),
            family: "Fira Code".into(),
            has_cjk: false,
        }),
    );
    assert_eq!(
        state.settings().en_font_family.as_deref(),
        Some("Fira Code")
    );
    assert_eq!(state.settings().zh_font_family.as_deref(), Some("KaiTi"));

    send(&mut state, Command::Settings(SettingsCommand::ResetFont));
    assert_eq!(state.settings().zh_font_family, None);
    assert_eq!(state.settings().en_font_family, None);
    assert_eq!(state.settings().font_size, FONT_SIZE_DEFAULT);
    assert!(state.settings().custom_font_paths.is_empty());
    assert_eq!(
        CORNER_RADIUS_DEFAULT,
        crate::model::settings::Settings::default().custom_corner_radius
    );
}
