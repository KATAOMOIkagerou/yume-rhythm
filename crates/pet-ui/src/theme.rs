use egui::{Color32, CornerRadius, Margin, Response, RichText, Sense, Stroke, StrokeKind, Ui, Vec2, Visuals};

// Cute Pastel Sakura / Cream Anime Palette
pub const BG_CANVAS: Color32 = Color32::from_rgb(253, 244, 248);       // #FDF4F8
pub const BG_HEADER: Color32 = Color32::from_rgb(251, 231, 241);       // #FBE7F1
pub const BG_CARD: Color32 = Color32::from_rgb(255, 255, 255);         // #FFFFFF
pub const BG_SOFT: Color32 = Color32::from_rgb(250, 240, 246);         // #FAF0F6
pub const BG_INPUT: Color32 = Color32::from_rgb(255, 250, 253);        // #FFFAFD

pub const BORDER_SOFT: Color32 = Color32::from_rgb(243, 215, 229);     // #F3D7E5
pub const BORDER_STRONG: Color32 = Color32::from_rgb(232, 186, 212);   // #E8BAD4

pub const ACCENT_PINK: Color32 = Color32::from_rgb(244, 114, 182);     // #F472B6
pub const ACCENT_PINK_DEEP: Color32 = Color32::from_rgb(219, 39, 119); // #DB2777
pub const ACCENT_PINK_SOFT: Color32 = Color32::from_rgb(252, 231, 243);// #FCE7F3

pub const ACCENT_PURPLE: Color32 = Color32::from_rgb(139, 92, 246);    // #8B5CF6
pub const ACCENT_PURPLE_SOFT: Color32 = Color32::from_rgb(237, 233, 254); // #EDE9FE

pub const ACCENT_MINT: Color32 = Color32::from_rgb(16, 185, 129);      // #10B981
pub const ACCENT_MINT_SOFT: Color32 = Color32::from_rgb(209, 250, 229);// #D1FAE5

pub const ACCENT_AMBER: Color32 = Color32::from_rgb(217, 119, 6);      // #D97706
pub const ACCENT_AMBER_SOFT: Color32 = Color32::from_rgb(254, 243, 199); // #FEF3C7

pub const ACCENT_CORAL: Color32 = Color32::from_rgb(239, 68, 68);      // #EF4444
pub const ACCENT_CORAL_SOFT: Color32 = Color32::from_rgb(254, 226, 226); // #FEE2E2

pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(59, 40, 68);       // #3B2844
pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(126, 104, 136);  // #7E6888
pub const TEXT_MUTED: Color32 = Color32::from_rgb(168, 149, 178);      // #A895B2

pub fn apply_pastel_theme(ctx: &egui::Context) {
    let mut visuals = Visuals::light();
    visuals.panel_fill = BG_CANVAS;
    visuals.window_fill = BG_CARD;
    visuals.extreme_bg_color = BG_INPUT;
    visuals.faint_bg_color = BG_SOFT;
    visuals.override_text_color = Some(TEXT_PRIMARY);
    visuals.selection.bg_fill = ACCENT_PINK_SOFT;
    visuals.selection.stroke = Stroke::new(1.5_f32, ACCENT_PINK);
    visuals.window_corner_radius = CornerRadius::same(16);
    visuals.window_stroke = Stroke::new(1.0_f32, BORDER_SOFT);

    visuals.widgets.noninteractive.bg_fill = BG_SOFT;
    visuals.widgets.noninteractive.weak_bg_fill = BG_SOFT;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER_SOFT);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, TEXT_PRIMARY);
    visuals.widgets.noninteractive.corner_radius = CornerRadius::same(10);

    visuals.widgets.inactive.bg_fill = BG_SOFT;
    visuals.widgets.inactive.weak_bg_fill = BG_SOFT;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, BORDER_SOFT);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.2_f32, TEXT_PRIMARY);
    visuals.widgets.inactive.corner_radius = CornerRadius::same(10);

    visuals.widgets.hovered.bg_fill = ACCENT_PINK_SOFT;
    visuals.widgets.hovered.weak_bg_fill = ACCENT_PINK_SOFT;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.2_f32, ACCENT_PINK);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.5_f32, ACCENT_PINK_DEEP);
    visuals.widgets.hovered.corner_radius = CornerRadius::same(10);

    visuals.widgets.active.bg_fill = ACCENT_PINK;
    visuals.widgets.active.weak_bg_fill = ACCENT_PINK;
    visuals.widgets.active.bg_stroke = Stroke::new(1.5_f32, ACCENT_PINK_DEEP);
    visuals.widgets.active.fg_stroke = Stroke::new(1.5_f32, Color32::WHITE);
    visuals.widgets.active.corner_radius = CornerRadius::same(10);

    visuals.slider_trailing_fill = true;

    let mut style = (*ctx.style()).clone();
    style.visuals = visuals;
    style.spacing.item_spacing = Vec2::new(8.0, 8.0);
    style.spacing.button_padding = Vec2::new(10.0, 6.0);
    style.spacing.slider_width = 160.0;
    style.spacing.scroll.bar_width = 6.0;
    ctx.set_style(style);
}

pub fn card_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(BG_CARD)
        .stroke(Stroke::new(1.0_f32, BORDER_SOFT))
        .corner_radius(CornerRadius::same(14))
        .inner_margin(Margin::symmetric(14, 12))
}

pub fn soft_panel_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(BG_SOFT)
        .stroke(Stroke::new(1.0_f32, BORDER_SOFT))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::symmetric(10, 8))
}

pub fn alert_frame(fill: Color32, border: Color32) -> egui::Frame {
    egui::Frame::new()
        .fill(fill)
        .stroke(Stroke::new(1.0_f32, border))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::symmetric(10, 8))
}

pub fn section_title(ui: &mut Ui, title: &str, subtitle: Option<&str>) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(title)
                .size(14.5)
                .strong()
                .color(TEXT_PRIMARY),
        );
        if let Some(sub) = subtitle {
            ui.label(RichText::new(sub).size(11.5).color(TEXT_SECONDARY));
        }
    });
}

pub fn badge(ui: &mut Ui, text: &str, fill: Color32, text_color: Color32) {
    egui::Frame::new()
        .fill(fill)
        .corner_radius(CornerRadius::same(99))
        .inner_margin(Margin::symmetric(8, 2))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(11.0).strong().color(text_color));
        });
}

pub fn pill_button(ui: &mut Ui, label: &str, active: bool, accent: Color32, min_size: Vec2) -> Response {
    let (fill, text_color, stroke) = if active {
        (accent, Color32::WHITE, Stroke::new(1.2_f32, accent))
    } else {
        (BG_SOFT, TEXT_PRIMARY, Stroke::new(1.0_f32, BORDER_SOFT))
    };
    ui.add(
        egui::Button::new(RichText::new(label).size(13.0).strong().color(text_color))
            .fill(fill)
            .stroke(stroke)
            .corner_radius(CornerRadius::same(10))
            .min_size(min_size),
    )
}

pub fn primary_button(ui: &mut Ui, label: &str, fill: Color32, min_size: Vec2) -> Response {
    ui.add(
        egui::Button::new(RichText::new(label).size(13.5).strong().color(Color32::WHITE))
            .fill(fill)
            .stroke(Stroke::NONE)
            .corner_radius(CornerRadius::same(11))
            .min_size(min_size),
    )
}

pub fn secondary_button(ui: &mut Ui, label: &str, min_size: Vec2) -> Response {
    ui.add(
        egui::Button::new(RichText::new(label).size(12.5).color(TEXT_PRIMARY))
            .fill(BG_SOFT)
            .stroke(Stroke::new(1.0_f32, BORDER_SOFT))
            .corner_radius(CornerRadius::same(10))
            .min_size(min_size),
    )
}

/// Custom-painted pastel toggle switch.
pub fn toggle_switch(ui: &mut Ui, on: bool, enabled: bool) -> Response {
    let desired_size = Vec2::new(40.0, 22.0);
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(desired_size, sense);

    if ui.is_rect_visible(rect) {
        let radius = rect.height() * 0.5;
        let bg = if !enabled {
            Color32::from_rgb(233, 224, 232)
        } else if on {
            ACCENT_PINK
        } else {
            Color32::from_rgb(219, 203, 217)
        };
        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::same(radius as u8), bg);
        if response.hovered() && enabled {
            painter.rect_stroke(
                rect,
                CornerRadius::same(radius as u8),
                Stroke::new(1.2_f32, ACCENT_PINK_DEEP),
                StrokeKind::Outside,
            );
        }

        let knob_r = radius - 3.0;
        let knob_x = if on {
            rect.right() - radius
        } else {
            rect.left() + radius
        };
        let knob_color = if enabled {
            Color32::WHITE
        } else {
            Color32::from_rgb(245, 240, 245)
        };
        painter.circle_filled(egui::pos2(knob_x, rect.center().y), knob_r, knob_color);
    }

    response
}
