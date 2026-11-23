//! 将 `view::View` 结构体和调色板转换为 Slint 生成类型并推送到 `Theme` / `Store`。

use crate::ui::{self, Theme};
use crate::view::{self, View};
use slint::Color;
use yume_core::model::theme::{Rgb, Rgba, ThemePalette};

fn to_color(c: Rgb) -> Color {
    Color::from_rgb_u8(c.r, c.g, c.b)
}

fn to_rgba(c: Rgba) -> Color {
    Color::from_argb_u8(c.a, c.r, c.g, c.b)
}

pub(super) fn apply_palette(theme: &Theme<'_>, p: ThemePalette) {
    theme.set_bg(to_color(p.bg));
    theme.set_surface(to_color(p.surface));
    theme.set_surface_alt(to_color(p.surface_alt));
    theme.set_surface_hover(to_color(p.surface_hover));
    theme.set_border(to_color(p.border));
    theme.set_divider(to_color(p.divider));
    theme.set_text(to_color(p.text));
    theme.set_text_secondary(to_color(p.text_secondary));
    theme.set_text_muted(to_color(p.text_muted));
    theme.set_text_on_accent(to_color(p.text_on_accent));
    theme.set_accent(to_color(p.accent));
    theme.set_accent_hover(to_color(p.accent_hover));
    theme.set_accent_pressed(to_color(p.accent_pressed));
    theme.set_accent_soft(to_color(p.accent_soft));
    theme.set_accent_strong_text(to_color(p.accent_strong_text));
    theme.set_violet(to_color(p.violet));
    theme.set_violet_soft(to_color(p.violet_soft));
    theme.set_green(to_color(p.green));
    theme.set_green_soft(to_color(p.green_soft));
    theme.set_amber(to_color(p.amber));
    theme.set_amber_soft(to_color(p.amber_soft));
    theme.set_red(to_color(p.red));
    theme.set_red_soft(to_color(p.red_soft));
    theme.set_scrim(to_rgba(p.scrim));
}

pub(super) fn header(v: &View) -> ui::HeaderView {
    ui::HeaderView {
        character: v.header.character.as_str().into(),
        audio_text: v.header.audio_text.as_str().into(),
        audio_active: v.header.audio_active,
        locked: v.header.locked,
    }
}

pub(super) fn focus(v: &View) -> ui::FocusView {
    let f = &v.focus;
    ui::FocusView {
        study: f.study,
        countdown: f.countdown,
        mode_locked: f.mode_locked,
        minutes: f.minutes,
        minutes_editable: f.minutes_editable,
        time_text: f.time_text.as_str().into(),
        status_text: f.status_text.as_str().into(),
        phase: f.phase as i32,
        progress: f.progress,
        show_progress: f.show_progress,
        can_reset: f.can_reset,
    }
}

pub(super) fn todo(v: &View) -> ui::TodoView {
    ui::TodoView {
        name: v.todo.name.as_str().into(),
        can_remove: v.todo.can_remove,
        done_count: v.todo.done_count,
        total: v.todo.total,
        md_bound: v.todo.md_bound,
        md_label: v.todo.md_label.as_str().into(),
        md_error: v.todo.md_error.as_str().into(),
    }
}

pub(super) fn todo_row(r: &view::TodoRow) -> ui::TodoRow {
    ui::TodoRow {
        id: i32::try_from(r.id).unwrap_or(i32::MAX),
        text: r.text.as_str().into(),
        done: r.done,
    }
}

pub(super) fn emotion(v: &View) -> ui::EmotionView {
    let e = &v.emotion;
    ui::EmotionView {
        current_name: e.current_name.as_str().into(),
        current_random: e.current_random,
        random_enabled: e.random_enabled,
        random_interval: e.random_interval,
        doze_note: e.doze_note,
    }
}

pub(super) fn chip(c: &view::Chip) -> ui::ExpressionChip {
    ui::ExpressionChip {
        id: c.id.as_str().into(),
        name: c.name.as_str().into(),
        active: c.active,
        enabled: c.enabled,
    }
}

pub(super) fn music(v: &View) -> ui::MusicView {
    let m = &v.music;
    ui::MusicView {
        phase: m.phase as i32,
        status_text: m.status_text.as_str().into(),
        source_text: m.source_text.as_str().into(),
        button_text: m.button_text.as_str().into(),
        show_bpm: m.show_bpm,
        bpm_text: m.bpm_text.as_str().into(),
        beat_locked: m.beat_locked,
        beat_text: m.beat_text.as_str().into(),
        level: m.level,
        bass: m.bass,
        can_shift: m.can_shift,
        silence_hint: m.silence_hint,
        error_text: m.error_text.as_str().into(),
        device_index: m.device_index,
        auto_follow: m.auto_follow,
    }
}

pub(super) fn accent_preset(opt: &view::AccentPresetView) -> ui::AccentOption {
    ui::AccentOption {
        key: opt.key.as_str().into(),
        name: opt.name.as_str().into(),
        swatch: to_color(opt.swatch),
        active: opt.active,
    }
}

pub(super) fn shortcut_item(item: &view::ShortcutItem) -> ui::ShortcutItem {
    ui::ShortcutItem {
        id: item.id.as_str().into(),
        name: item.name.as_str().into(),
        key_text: item.key_text.as_str().into(),
        is_global: item.is_global,
        can_clear: item.can_clear,
        failed: item.failed,
    }
}

pub(super) fn settings(v: &View) -> ui::SettingsView {
    let s = &v.settings;
    ui::SettingsView {
        end_sound: s.end_sound,
        pet_scale: s.pet_scale,
        pet_scale_text: s.pet_scale_text.as_str().into(),
        pet_opacity: s.pet_opacity,
        pet_opacity_text: s.pet_opacity_text.as_str().into(),
        console_opacity: s.console_opacity,
        console_opacity_text: s.console_opacity_text.as_str().into(),
        background_index: s.background_index,
        custom_background: s.custom_background.as_str().into(),
        bg_error: s.bg_error,
        accent_custom_active: s.accent_custom_active,
        custom_accent: s.custom_accent.as_str().into(),
        accent_error: s.accent_error,
        has_bg_image: s.has_bg_image,
        bg_image_name: s.bg_image_name.as_str().into(),
        bg_dim: s.bg_dim,
        bg_dim_text: s.bg_dim_text.as_str().into(),
        bg_image_error: s.bg_image_error.as_str().into(),
        shortcut_error: s.shortcut_error.as_str().into(),
        font_index: s.font_index,
        font_status: s.font_status.as_str().into(),
        font_failed: s.font_failed,
        locked: s.locked,
        hide_on_fullscreen: s.hide_on_fullscreen,
        autostart: s.autostart,
        character_index: s.character_index,
        character_status: s.character_status.as_str().into(),
        character_failed: s.character_failed,
        version: s.version.as_str().into(),
    }
}
