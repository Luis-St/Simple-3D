//! The row of tabs itself.

use crate::app::App;
use crate::theme::{self, metric, token};

/// The row of open documents: the top of the workspace, between the docks and
/// over the viewport, so a tab sits above the model it holds.
///
/// Drawn by hand rather than out of widgets so a tab can be a shape -- the
/// active one lit along its top edge and joined to the workspace below it --
/// which is what makes the row readable at a glance.
pub fn show(app: &mut App, ctx: &egui::Context) {
    let frame = egui::Frame::NONE.fill(token::SURFACE_1);
    egui::TopBottomPanel::top("tabs").frame(frame).exact_height(metric::TAB_BAR).show(ctx, |ui| {
        ui.painter().hline(
            ui.max_rect().x_range(),
            ui.max_rect().bottom() - 0.5,
            egui::Stroke::new(1.0_f32, token::SURFACE_3),
        );
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(1.0, 0.0);
            let mut clicked: Option<usize> = None;
            let mut closed: Option<usize> = None;
            for index in 0..app.tab_count() {
                let (name, unsaved) = app.tab_summary(index);
                match tab(ui, &name, unsaved, index == app.active) {
                    Some(Hit::Pick) => clicked = Some(index),
                    Some(Hit::Close) => closed = Some(index),
                    None => {}
                }
            }
            if plus(ui) {
                app.run(simple3d_core::keymap::Command::New);
            }
            // After the row, so closing a tab cannot renumber the ones still
            // being drawn.
            if let Some(index) = clicked {
                app.activate_tab(index);
            }
            if let Some(index) = closed {
                app.close_tab(index);
            }
        });
    });
}

/// What a click on a tab was.
pub(crate) enum Hit {
    Pick,
    Close,
}

/// One tab. Returns what was clicked on it, if anything.
pub(crate) fn tab(ui: &mut egui::Ui, name: &str, unsaved: bool, active: bool) -> Option<Hit> {
    const MIN: f32 = 96.0;
    const MAX: f32 = 220.0;
    const CLOSE: f32 = 16.0;

    let font = egui::FontId::proportional(theme::font::VALUE);
    let label = if unsaved { format!("{name} \u{2022}") } else { name.to_string() };
    let text_width = ui.fonts(|fonts| fonts.layout_no_wrap(label.clone(), font.clone(), token::TEXT_HI).size().x);
    let width = (text_width + CLOSE + 24.0).clamp(MIN, MAX.min(ui.available_width().max(MIN)));
    let height = ui.available_height();
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());

    let fill = if active {
        token::SURFACE_0B
    } else if response.hovered() {
        token::SURFACE_2
    } else {
        token::SURFACE_1
    };
    ui.painter().rect_filled(rect, 0.0, fill);
    if active {
        // The lit top edge, and no rule along the bottom: the active tab is the
        // workspace's own top, not a button sitting above it.
        ui.painter().hline(rect.x_range(), rect.top() + 1.0, egui::Stroke::new(2.0_f32, token::ACCENT));
    } else {
        ui.painter().hline(rect.x_range(), rect.bottom() - 0.5, egui::Stroke::new(1.0_f32, token::SURFACE_3));
    }
    ui.painter().vline(rect.right() - 0.5, rect.y_range(), egui::Stroke::new(1.0_f32, token::SURFACE_0));

    let close_rect =
        egui::Rect::from_center_size(egui::pos2(rect.right() - 13.0, rect.center().y), egui::Vec2::splat(CLOSE));
    let text_colour = if active { token::TEXT_HI } else { token::TEXT_LO };
    let mut job = egui::text::LayoutJob::simple_singleline(label, font, text_colour);
    job.wrap.max_width = (close_rect.left() - rect.left() - 16.0).max(8.0);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    let galley = ui.fonts(|fonts| fonts.layout_job(job));
    ui.painter().galley(egui::pos2(rect.left() + 10.0, rect.center().y - galley.size().y * 0.5), galley, text_colour);

    let close = ui.interact(close_rect, response.id.with("close"), egui::Sense::click());
    if close.hovered() {
        ui.painter().rect_filled(close_rect, 3.0, token::SURFACE_3);
    }
    cross(ui.painter(), close_rect.center(), if close.hovered() { token::TEXT_HI } else { token::TEXT_LO });
    let response =
        response.on_hover_text(if unsaved { format!("{name} \u{2022} unsaved changes") } else { name.to_string() });

    if close.clicked() || response.middle_clicked() {
        return Some(Hit::Close);
    }
    if response.clicked() {
        return Some(Hit::Pick);
    }
    None
}

/// The button at the end of the row: another document.
pub(crate) fn plus(ui: &mut egui::Ui) -> bool {
    let size = egui::vec2(28.0, ui.available_height());
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let colour = if response.hovered() { token::TEXT_HI } else { token::TEXT_LO };
    if response.hovered() {
        ui.painter().rect_filled(rect, 0.0, token::SURFACE_2);
    }
    let centre = rect.center();
    let stroke = egui::Stroke::new(1.4_f32, colour);
    ui.painter().hline(centre.x - 5.0..=centre.x + 5.0, centre.y, stroke);
    ui.painter().vline(centre.x, centre.y - 5.0..=centre.y + 5.0, stroke);
    response.on_hover_text("New document").clicked()
}

/// The close cross, drawn rather than typed: a glyph would depend on the font
/// having it, and would sit off centre in most that do.
pub(crate) fn cross(painter: &egui::Painter, centre: egui::Pos2, colour: egui::Color32) {
    let r = 3.5;
    let stroke = egui::Stroke::new(1.3_f32, colour);
    painter.line_segment([centre + egui::vec2(-r, -r), centre + egui::vec2(r, r)], stroke);
    painter.line_segment([centre + egui::vec2(r, -r), centre + egui::vec2(-r, r)], stroke);
}
