//! Colours, fonts and small styled widgets shared by every page.
#![expect(
    unused_results,
    reason = "egui painter and widget calls return handles that drawing code does not need"
)]

use eframe::egui::{
    Align2, Color32, Context, CornerRadius, FontId, Frame, Margin, Sense, Stroke, StrokeKind,
    TextStyle, Theme, Ui, Vec2, Visuals,
};

pub const BG: Color32 = Color32::from_rgb(17, 18, 28);
pub const PANEL_BG: Color32 = Color32::from_rgb(24, 25, 40);
pub const PANEL_BORDER: Color32 = Color32::from_rgb(44, 46, 74);
pub const TAB_ACTIVE_BG: Color32 = Color32::from_rgb(54, 44, 82);
pub const TAB_INACTIVE_BG: Color32 = Color32::from_rgb(20, 21, 33);

pub const ACCENT_LILAC: Color32 = Color32::from_rgb(208, 188, 255);
pub const ACCENT_PERIWINKLE: Color32 = Color32::from_rgb(180, 198, 255);
pub const ACCENT_AMBER: Color32 = Color32::from_rgb(245, 158, 11);
pub const ACCENT_GREEN: Color32 = Color32::from_rgb(16, 185, 129);

pub const TEXT_MAIN: Color32 = Color32::from_rgb(241, 242, 246);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(147, 150, 172);

pub const STATE_RUNNING: Color32 = ACCENT_GREEN;
pub const STATE_PAUSED: Color32 = ACCENT_AMBER;
pub const STATE_DONE: Color32 = ACCENT_LILAC;
pub const STATE_IDLE: Color32 = Color32::from_rgb(120, 125, 145);

pub fn card_frame() -> Frame {
    Frame::new()
        .fill(PANEL_BG)
        .stroke(Stroke::new(1.0, PANEL_BORDER))
        .corner_radius(CornerRadius::same(6_u8))
        .inner_margin(Margin::symmetric(14, 14))
}

pub fn status_badge(ui: &mut Ui, text: &str, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(98.0, 26.0), Sense::hover());
    ui.painter().rect_filled(
        rect,
        CornerRadius::same(4_u8),
        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 35_u8),
    );
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(4_u8),
        Stroke::new(1.0, color),
        StrokeKind::Inside,
    );
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        text,
        FontId::monospace(12.0),
        color,
    );
}

pub fn configure_theme(ctx: &Context) {
    ctx.set_theme(Theme::Dark);

    let mut visuals = Visuals::dark();
    visuals.panel_fill = BG;
    visuals.window_fill = PANEL_BG;
    visuals.extreme_bg_color = BG;

    // Zero hover/active widget expansion to eliminate layout jitter
    visuals.widgets.noninteractive.expansion = 0.0;
    visuals.widgets.inactive.expansion = 0.0;
    visuals.widgets.hovered.expansion = 0.0;
    visuals.widgets.active.expansion = 0.0;
    visuals.widgets.open.expansion = 0.0;

    visuals.widgets.noninteractive.bg_fill = PANEL_BG;
    visuals.widgets.inactive.bg_fill = TAB_INACTIVE_BG;
    visuals.widgets.hovered.bg_fill = TAB_ACTIVE_BG;
    visuals.widgets.active.bg_fill = TAB_ACTIVE_BG;
    visuals.widgets.open.bg_fill = TAB_ACTIVE_BG;

    // Fixed stroke width 1.0 ensures zero size jitter on hover
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, PANEL_BORDER);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, PANEL_BORDER);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT_LILAC);
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT_LILAC);
    visuals.widgets.open.bg_stroke = Stroke::new(1.0, ACCENT_LILAC);

    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT_MAIN);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, ACCENT_LILAC);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, TEXT_MAIN);

    visuals.selection.bg_fill = TAB_ACTIVE_BG;
    visuals.selection.stroke = Stroke::new(1.0, ACCENT_LILAC);

    let visuals_clone = visuals.clone();
    ctx.all_styles_mut(move |style| {
        style.visuals = visuals_clone.clone();
        style.spacing.item_spacing = Vec2::new(10.0, 10.0);
        style.spacing.button_padding = Vec2::new(12.0, 7.0);
        style.text_styles.extend([
            (TextStyle::Heading, FontId::monospace(17.0)),
            (TextStyle::Body, FontId::monospace(13.5)),
            (TextStyle::Monospace, FontId::monospace(13.5)),
            (TextStyle::Button, FontId::monospace(13.0)),
            (TextStyle::Small, FontId::monospace(11.5)),
        ]);
    });
    ctx.set_visuals(visuals);
}
