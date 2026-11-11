//! Colours, fonts and small styled widgets shared by every page.
#![expect(
    unused_results,
    reason = "egui painter and widget calls return handles that drawing code does not need"
)]

use eframe::egui::{
    Align2, Button, Color32, Context, CornerRadius, FontId, Frame, Margin, Response, RichText,
    Sense, Stroke, StrokeKind, TextStyle, Theme as EguiTheme, Ui, Vec2, Visuals,
};

/// Zero-sized type grouping application palette constants, typography and widget helpers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Theme;

impl Theme {
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

    pub const STATE_RUNNING: Color32 = Self::ACCENT_GREEN;
    pub const STATE_PAUSED: Color32 = Self::ACCENT_AMBER;
    pub const STATE_DONE: Color32 = Self::ACCENT_LILAC;
    pub const STATE_IDLE: Color32 = Color32::from_rgb(120, 125, 145);

    /// Container frame for the top bar header, spanning window edge to edge.
    pub fn top_bar_frame() -> Frame {
        Frame::new()
            .fill(Self::PANEL_BG)
            .stroke(Stroke::new(1.0, Self::PANEL_BORDER))
            .corner_radius(CornerRadius::ZERO)
            .inner_margin(Margin::symmetric(14, 8))
    }

    /// Standard card frame for dashboard panels and widgets.
    pub fn card_frame() -> Frame {
        Frame::new()
            .fill(Self::PANEL_BG)
            .stroke(Stroke::new(1.0, Self::PANEL_BORDER))
            .corner_radius(CornerRadius::same(6_u8))
            .inner_margin(Margin::symmetric(14, 14))
    }

    /// Standard section heading typography.
    #[must_use]
    pub fn heading(text: impl Into<String>, color: Color32) -> RichText {
        RichText::new(text.into())
            .monospace()
            .size(12.5)
            .strong()
            .color(color)
    }

    /// Standard subsection or parameter group heading typography.
    #[must_use]
    pub fn subheading(text: impl Into<String>, color: Color32) -> RichText {
        RichText::new(text.into())
            .monospace()
            .size(11.0)
            .strong()
            .color(color)
    }

    /// Standard parameter caption label typography.
    #[must_use]
    pub fn caption(text: impl Into<String>) -> RichText {
        RichText::new(text.into())
            .monospace()
            .size(10.5)
            .color(Self::TEXT_MUTED)
    }

    /// Standard technical note or footnote typography.
    #[must_use]
    pub fn note(text: impl Into<String>) -> RichText {
        RichText::new(text.into())
            .monospace()
            .size(9.5)
            .color(Self::TEXT_MUTED)
    }

    /// Standard secondary action button in inactive panel styling.
    pub fn secondary_button(text: impl Into<String>, min_size: Vec2) -> Button<'static> {
        Button::new(RichText::new(text.into()).monospace().size(11.5))
            .fill(Self::TAB_INACTIVE_BG)
            .stroke(Stroke::new(1.0, Self::PANEL_BORDER))
            .min_size(min_size)
    }

    /// Standard segmented button (choice toggle) with consistent styling, stroke, fill and height.
    pub fn segmented_button(
        text: impl Into<String>,
        selected: bool,
        min_size: Vec2,
    ) -> Button<'static> {
        Self::segmented_button_sized(text, selected, min_size, 11.0, false)
    }

    /// Standard segmented button with custom font size and weight.
    pub fn segmented_button_sized(
        text: impl Into<String>,
        selected: bool,
        min_size: Vec2,
        font_size: f32,
        strong: bool,
    ) -> Button<'static> {
        let (fill, accent) = if selected {
            (Self::TAB_ACTIVE_BG, Self::ACCENT_LILAC)
        } else {
            (Self::TAB_INACTIVE_BG, Self::PANEL_BORDER)
        };
        let color = if selected {
            Self::ACCENT_LILAC
        } else {
            Self::TEXT_MUTED
        };
        let mut label = RichText::new(text.into())
            .monospace()
            .size(font_size)
            .color(color);
        if strong || selected {
            label = label.strong();
        }
        Button::new(label)
            .fill(fill)
            .stroke(Stroke::new(1.0, accent))
            .min_size(min_size)
    }

    /// Standard primary action button (e.g. START, START STEP) in accent green.
    pub fn primary_button(text: impl Into<String>, min_size: Vec2) -> Button<'static> {
        Button::new(
            RichText::new(text.into())
                .monospace()
                .size(12.0)
                .strong()
                .color(Color32::BLACK),
        )
        .fill(Self::ACCENT_GREEN)
        .min_size(min_size)
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

    pub fn device_badge(ui: &mut Ui, text: &str, color: Color32) -> Response {
        let font_id = FontId::monospace(12.0);
        let text_w = ui
            .painter()
            .layout_no_wrap(text.to_owned(), font_id.clone(), color)
            .size()
            .x;
        let w = (text_w + 18.0).max(70.0);
        let (rect, response) = ui.allocate_exact_size(Vec2::new(w, 26.0), Sense::hover());
        ui.painter().rect_filled(
            rect,
            CornerRadius::same(4_u8),
            Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 25_u8),
        );
        ui.painter().rect_stroke(
            rect,
            CornerRadius::same(4_u8),
            Stroke::new(
                1.0,
                Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 90_u8),
            ),
            StrokeKind::Inside,
        );
        ui.painter()
            .text(rect.center(), Align2::CENTER_CENTER, text, font_id, color);
        response
    }

    pub fn configure(ctx: &Context) {
        ctx.set_theme(EguiTheme::Dark);

        let mut visuals = Visuals::dark();
        visuals.panel_fill = Self::BG;
        visuals.window_fill = Self::PANEL_BG;
        visuals.extreme_bg_color = Self::BG;

        // Zero hover/active widget expansion to eliminate layout jitter
        visuals.widgets.noninteractive.expansion = 0.0;
        visuals.widgets.inactive.expansion = 0.0;
        visuals.widgets.hovered.expansion = 0.0;
        visuals.widgets.active.expansion = 0.0;
        visuals.widgets.open.expansion = 0.0;

        visuals.widgets.noninteractive.bg_fill = Self::PANEL_BG;
        visuals.widgets.inactive.bg_fill = Self::TAB_INACTIVE_BG;
        visuals.widgets.hovered.bg_fill = Self::TAB_ACTIVE_BG;
        visuals.widgets.active.bg_fill = Self::TAB_ACTIVE_BG;
        visuals.widgets.open.bg_fill = Self::TAB_ACTIVE_BG;

        // Fixed stroke width 1.0 ensures zero size jitter on hover
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Self::PANEL_BORDER);
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, Self::PANEL_BORDER);
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, Self::ACCENT_LILAC);
        visuals.widgets.active.bg_stroke = Stroke::new(1.0, Self::ACCENT_LILAC);
        visuals.widgets.open.bg_stroke = Stroke::new(1.0, Self::ACCENT_LILAC);

        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, Self::TEXT_MAIN);
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, Self::ACCENT_LILAC);
        visuals.widgets.active.fg_stroke = Stroke::new(1.0, Self::TEXT_MAIN);

        visuals.selection.bg_fill = Self::TAB_ACTIVE_BG;
        visuals.selection.stroke = Stroke::new(1.0, Self::ACCENT_LILAC);

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
}
