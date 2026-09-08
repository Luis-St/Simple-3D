//! Applying the theme to egui's own style.

use super::*;
use egui::{CornerRadius, Margin, Stroke, Vec2};

/// Install the palette and the metrics on a context. Called once, at startup.
pub fn apply(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();

    style.spacing.item_spacing = Vec2::new(metric::GAP, metric::GAP);
    style.spacing.button_padding = Vec2::new(6.0, 2.0);
    style.spacing.interact_size = Vec2::new(24.0, metric::INPUT_ROW);
    style.spacing.icon_width = 13.0;
    style.spacing.icon_width_inner = 7.0;
    style.spacing.icon_spacing = 4.0;
    style.spacing.indent = 14.0;
    style.spacing.menu_margin = Margin::symmetric(4, 4);
    style.spacing.window_margin = Margin::same(10);
    style.spacing.combo_width = 56.0;
    style.spacing.scroll.bar_width = 8.0;
    style.spacing.scroll.floating = false;
    style.spacing.scroll.bar_inner_margin = 2.0;
    // The handle takes the widget *fill*, not its foreground: egui's default
    // paints it in the text colour, which puts a bright bar down the side of
    // every list.
    style.spacing.scroll.foreground_color = false;
    style.spacing.scroll.bar_outer_margin = 0.0;

    for (text_style, id) in [
        (egui::TextStyle::Small, egui::FontId::proportional(font::SMALL)),
        (egui::TextStyle::Body, egui::FontId::proportional(font::LABEL)),
        (egui::TextStyle::Button, egui::FontId::proportional(font::LABEL)),
        (egui::TextStyle::Heading, egui::FontId::proportional(font::TITLE)),
        (egui::TextStyle::Monospace, egui::FontId::monospace(font::VALUE)),
    ] {
        style.text_styles.insert(text_style, id);
    }

    let mut visuals = egui::Visuals::dark();
    visuals.dark_mode = true;
    visuals.panel_fill = token::SURFACE_1;
    visuals.window_fill = token::SURFACE_1;
    visuals.faint_bg_color = token::SURFACE_2;
    visuals.extreme_bg_color = token::SURFACE_2;
    visuals.code_bg_color = token::SURFACE_2;
    visuals.window_stroke = Stroke::new(1.0_f32, token::SURFACE_3);
    visuals.window_corner_radius = CornerRadius::same(3);
    visuals.menu_corner_radius = CornerRadius::same(3);
    // Panels are separated by lines, so nothing needs a shadow to lift off the
    // surface behind it.
    visuals.window_shadow = egui::epaint::Shadow::NONE;
    visuals.popup_shadow = egui::epaint::Shadow::NONE;
    visuals.warn_fg_color = token::ACCENT;
    visuals.error_fg_color = token::DANGER;
    visuals.hyperlink_color = token::MEASURE;
    visuals.weak_text_color = Some(token::TEXT_LO);
    visuals.button_frame = true;
    visuals.collapsing_header_frame = false;
    visuals.indent_has_left_vline = false;
    visuals.striped = false;
    visuals.slider_trailing_fill = true;
    visuals.text_cursor.stroke = Stroke::new(1.0_f32, token::ACCENT);
    visuals.selection.bg_fill = token::ACCENT.gamma_multiply(0.30);
    visuals.selection.stroke = Stroke::new(1.0_f32, token::ACCENT);
    visuals.clip_rect_margin = 1.0;
    visuals.resize_corner_size = 8.0;

    let radius = CornerRadius::same(3);
    // Rest.
    visuals.widgets.noninteractive.bg_fill = token::SURFACE_1;
    visuals.widgets.noninteractive.weak_bg_fill = token::SURFACE_1;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, token::SURFACE_3);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, token::TEXT_LO);
    visuals.widgets.noninteractive.corner_radius = radius;
    visuals.widgets.noninteractive.expansion = 0.0;

    visuals.widgets.inactive.bg_fill = token::SURFACE_2;
    visuals.widgets.inactive.weak_bg_fill = token::SURFACE_2;
    visuals.widgets.inactive.bg_stroke = Stroke::NONE;
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, token::TEXT_HI);
    visuals.widgets.inactive.corner_radius = radius;
    visuals.widgets.inactive.expansion = 0.0;

    // Hover is a surface change, not a tint of the text: the row lights up and
    // the label stays exactly where it was.
    visuals.widgets.hovered.bg_fill = token::SURFACE_3;
    visuals.widgets.hovered.weak_bg_fill = token::SURFACE_3;
    visuals.widgets.hovered.bg_stroke = Stroke::NONE;
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, token::TEXT_HI);
    visuals.widgets.hovered.corner_radius = radius;
    visuals.widgets.hovered.expansion = 0.0;

    // Pressed and active are an accent fill, not a tint -- the design's one
    // firm rule about the active tool.
    visuals.widgets.active.bg_fill = token::ACCENT;
    visuals.widgets.active.weak_bg_fill = token::ACCENT;
    visuals.widgets.active.bg_stroke = Stroke::NONE;
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, token::SURFACE_0);
    visuals.widgets.active.corner_radius = radius;
    visuals.widgets.active.expansion = 0.0;

    // Keyboard focus is always visible: a 1 px accent ring, never suppressed.
    visuals.widgets.open.bg_fill = token::SURFACE_2;
    visuals.widgets.open.weak_bg_fill = token::SURFACE_2;
    visuals.widgets.open.bg_stroke = Stroke::new(1.0_f32, token::ACCENT);
    visuals.widgets.open.fg_stroke = Stroke::new(1.0_f32, token::TEXT_HI);
    visuals.widgets.open.corner_radius = radius;
    visuals.widgets.open.expansion = 0.0;

    // The scrollbar is furniture, not content: it lives in the divider grey and
    // only reaches text-lo when it is being dragged.
    visuals.widgets.noninteractive.bg_fill = token::SURFACE_1;
    visuals.disabled_alpha = 0.45;

    style.visuals = visuals;
    // Panels butt against each other; only the dock's own edge is draggable, and
    // it reads as a line rather than a handle.
    style.interaction.selectable_labels = false;
    ctx.set_style(style);
}
