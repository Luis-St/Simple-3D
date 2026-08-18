//! The visual language: one palette, one set of metrics, applied once at
//! startup so no widget has to name a colour of its own.
//!
//! The tokens below are the whole vocabulary. Amber is selection, cyan is
//! measurement, red is destruction -- and none of the three collides with the
//! X/Y/Z axis colours, which is the reason selection is not blue like every
//! other modeller's.

use egui::{Color32, CornerRadius, Margin, Stroke, Vec2};

/// One named colour. Everything drawn by the interface comes from here.
pub mod token {
    use egui::Color32;

    /// Viewport background (top of a subtle vertical gradient to [`SURFACE_0B`]).
    pub const SURFACE_0: Color32 = Color32::from_rgb(0x15, 0x18, 0x1C);
    /// The bottom of that gradient.
    pub const SURFACE_0B: Color32 = Color32::from_rgb(0x1B, 0x1F, 0x24);
    /// Dock background.
    pub const SURFACE_1: Color32 = Color32::from_rgb(0x1E, 0x22, 0x28);
    /// Panel headers, tool rail, input fields.
    pub const SURFACE_2: Color32 = Color32::from_rgb(0x26, 0x2B, 0x33);
    /// Hover, dividers, grid minor lines.
    pub const SURFACE_3: Color32 = Color32::from_rgb(0x32, 0x38, 0x44);

    /// Values, names.
    pub const TEXT_HI: Color32 = Color32::from_rgb(0xE6, 0xEA, 0xF0);
    /// Labels, units, disabled.
    pub const TEXT_LO: Color32 = Color32::from_rgb(0x8B, 0x95, 0xA5);

    /// Selection, active tool, focus ring -- a machined-brass amber.
    pub const ACCENT: Color32 = Color32::from_rgb(0xE8, 0xA3, 0x3D);
    /// Dimension readouts, the measure tool, snap indicators.
    pub const MEASURE: Color32 = Color32::from_rgb(0x4F, 0xC3, 0xD9);
    /// Difference operands, destructive actions, errors.
    pub const DANGER: Color32 = Color32::from_rgb(0xD4, 0x57, 0x4E);

    pub const AXIS_X: Color32 = Color32::from_rgb(0xD4, 0x57, 0x4E);
    pub const AXIS_Y: Color32 = Color32::from_rgb(0x6F, 0xBF, 0x5B);
    pub const AXIS_Z: Color32 = Color32::from_rgb(0x55, 0x90, 0xD9);
}

/// Row and control metrics. Density over comfort: this is a tool used for
/// hours, so rows are tight and there is no decorative whitespace.
pub mod metric {
    /// Height of one outliner row.
    pub const ROW: f32 = 22.0;
    /// Height of one input row.
    pub const INPUT_ROW: f32 = 24.0;
    /// Padding inside a panel.
    pub const PANEL_PAD: f32 = 8.0;
    /// Gap between adjacent controls.
    pub const GAP: f32 = 4.0;
    /// Width of the tool rail.
    pub const RAIL: f32 = 40.0;
    /// Height of the menu bar.
    pub const MENU_BAR: f32 = 32.0;
    /// Height of the status bar.
    pub const STATUS_BAR: f32 = 24.0;
    /// Side of the orientation cube in the viewport's bottom-right corner.
    pub const VIEW_CUBE: f32 = 72.0;
}

/// Type sizes. Labels are one step below values so a column of numbers reads
/// as the content and the words around it as the frame.
pub mod font {
    pub const LABEL: f32 = 12.0;
    pub const VALUE: f32 = 13.0;
    pub const HEADER: f32 = 12.0;
    pub const SMALL: f32 = 11.0;
}

/// A value: a name, a measurement, anything the user typed or the model owns.
pub fn value(text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text).size(font::VALUE).color(token::TEXT_HI)
}

/// A number. Tabular figures, so a column of them lines up and no digit shifts
/// width while it is being scrubbed -- the typographic tell that this
/// application is about measurement.
pub fn numeric(text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text).size(font::VALUE).monospace().color(token::TEXT_HI)
}

/// A quiet aside: a hint, a unit, a default nobody has overridden.
pub fn hint(text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text).size(font::SMALL).color(token::TEXT_LO)
}

/// A panel header, in the small-caps-with-tracking form the design calls for.
/// egui has no small caps, so the effect is made the way a typesetter without
/// the face would make it: upper case, one size down, letters spaced out.
pub fn header_text(name: &str) -> egui::RichText {
    let spaced: String = name.to_uppercase().chars().flat_map(|c| [c, '\u{2009}']).collect();
    egui::RichText::new(spaced.trim_end().to_string()).size(font::HEADER).color(token::TEXT_LO).strong()
}

/// Draw a panel header bar: the name on the left, whatever the caller wants on
/// the right. Returns the response of the whole bar so it can be clicked to
/// collapse.
pub fn panel_header(ui: &mut egui::Ui, name: &str, right: impl FnOnce(&mut egui::Ui)) -> egui::Response {
    let full = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(egui::vec2(full, metric::ROW), egui::Sense::click());
    ui.painter().rect_filled(rect, 0.0, token::SURFACE_2);
    ui.painter().hline(rect.x_range(), rect.bottom() - 0.5, Stroke::new(1.0_f32, token::SURFACE_3));
    let inner = rect.shrink2(egui::vec2(metric::PANEL_PAD, 0.0));
    let mut child =
        ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::left_to_right(egui::Align::Center)));
    child.add(egui::Label::new(header_text(name)).selectable(false));
    child.with_layout(egui::Layout::right_to_left(egui::Align::Center), right);
    response
}

/// The collapse triangle, drawn rather than typed. Points down when the thing
/// it opens is open, right when it is closed -- the convention every tree in
/// every file manager uses, which is why it needs no label.
pub fn twisty(painter: &egui::Painter, centre: egui::Pos2, open: bool, colour: Color32) {
    let r = 4.0;
    let points = if open {
        vec![
            egui::pos2(centre.x - r, centre.y - r * 0.5),
            egui::pos2(centre.x + r, centre.y - r * 0.5),
            egui::pos2(centre.x, centre.y + r * 0.7),
        ]
    } else {
        vec![
            egui::pos2(centre.x - r * 0.5, centre.y - r),
            egui::pos2(centre.x - r * 0.5, centre.y + r),
            egui::pos2(centre.x + r * 0.7, centre.y),
        ]
    };
    painter.add(egui::Shape::convex_polygon(points, colour, Stroke::NONE));
}

/// A 1 px divider between panels. Panels are separated by lines, never by
/// shadows.
pub fn divider(ui: &mut egui::Ui) {
    let full = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(full, 1.0), egui::Sense::hover());
    ui.painter().hline(rect.x_range(), rect.center().y, Stroke::new(1.0_f32, token::SURFACE_3));
}

/// The colour chip that stands in front of an axis field, so a row of three
/// numbers says which is which without spelling out X, Y and Z.
pub fn axis_colour(axis: usize) -> Color32 {
    match axis {
        0 => token::AXIS_X,
        1 => token::AXIS_Y,
        _ => token::AXIS_Z,
    }
}

/// Paint the small axis chip and return the width it consumed.
pub fn axis_chip(ui: &mut egui::Ui, axis: usize) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(3.0, metric::INPUT_ROW - 8.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(1), axis_colour(axis));
}

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
        (egui::TextStyle::Heading, egui::FontId::proportional(font::HEADER)),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_panel_header_reads_as_spaced_capitals() {
        let text = header_text("Dimensions");
        assert_eq!(text.text().replace('\u{2009}', ""), "DIMENSIONS");
        assert!(text.text().contains('\u{2009}'), "header lost its tracking");
        assert!(!text.text().ends_with('\u{2009}'), "trailing space would offset a right-aligned header");
    }

    #[test]
    fn every_axis_has_its_own_colour() {
        let colours = [axis_colour(0), axis_colour(1), axis_colour(2)];
        for (i, a) in colours.iter().enumerate() {
            for b in &colours[i + 1..] {
                assert_ne!(a, b, "two axes share a colour");
            }
            // Selection must never be mistaken for an axis handle, which is the
            // reason the accent is amber rather than blue.
            assert_ne!(*a, token::ACCENT, "an axis colour collides with the selection accent");
        }
    }
}
