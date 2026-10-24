//! The workplanes (issue 125), under the primitives: the scene's own ground first, then every
//! workplane put down in this component. A click makes one the plane shapes are added, moved and
//! snapped on; the pencil and bin at a row's right edge change and delete it. The list ends in a row
//! that puts a new one down.

use crate::app::App;
use crate::icon::{self, Glyph};
use crate::theme::{self, metric, token};
use simple3d_core::keymap::Command;
use simple3d_core::scene::WorkplaneId;

/// Side and spacing of a row's icons, as the outliner's switches.
const ICON: f32 = 16.0;
const ICON_STEP: f32 = 20.0;

/// A workplane's row, by its id rather than its place in the list.
pub(crate) fn row_id(plane: WorkplaneId) -> egui::Id {
    egui::Id::new(("workplane-row", plane))
}

/// A row's pencil, which changes the workplane.
pub(crate) fn edit_id(plane: WorkplaneId) -> egui::Id {
    row_id(plane).with("edit")
}

/// A row's bin, which deletes the workplane.
pub(crate) fn delete_id(plane: WorkplaneId) -> egui::Id {
    row_id(plane).with("delete")
}

/// What a row was asked to do this frame.
enum Asked {
    Activate(Option<WorkplaneId>),
    Edit(WorkplaneId),
    Delete(WorkplaneId),
    Add,
}

pub fn show_inside(app: &mut App, ui: &mut egui::Ui) {
    ui.spacing_mut().item_spacing = egui::vec2(metric::GAP, 0.0);
    ui.add_space(2.0);
    let (area, restore) = theme::list_scroll_area(ui);
    let mut asked = None;
    area.id_salt("workplanes").show(ui, |ui| {
        ui.set_style(restore);
        let active = app.scene.settings.active_workplane;
        let ground = "The scene's own ground grid, at the Document panel's spacing";
        if let Some(now) = row(ui, egui::Id::new("workplane-scene"), "Scene", active.is_none(), ground, None) {
            asked = Some(now);
        }
        for plane in &app.scene.settings.workplanes {
            let hover = "Click to work on it";
            let id = row_id(plane.id);
            if let Some(now) = row(ui, id, &plane.name, active == Some(plane.id), hover, Some(plane.id)) {
                asked = Some(now);
            }
        }
        if add_row(app, ui) {
            asked = Some(Asked::Add);
        }
    });
    match asked {
        Some(Asked::Activate(id)) => app.activate_workplane(id),
        Some(Asked::Edit(id)) => app.edit_workplane(id),
        Some(Asked::Delete(id)) => app.delete_workplane(id),
        Some(Asked::Add) => app.toggle_workplane_tool(),
        None => {}
    }
}

/// One row, marked when active, the way the outliner marks a selected row; `plane` is `None` for the
/// scene's ground, which cannot be changed or deleted.
fn row(
    ui: &mut egui::Ui,
    id: egui::Id,
    name: &str,
    active: bool,
    hover: &str,
    plane: Option<WorkplaneId>,
) -> Option<Asked> {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, metric::ROW), egui::Sense::click());
    let response = ui.interact(rect, id, egui::Sense::click()).union(response).on_hover_text(hover);
    let painter = ui.painter();
    if active {
        painter.rect_filled(rect, 0.0, token::ACCENT.gamma_multiply(0.13));
        let bar = egui::Rect::from_min_size(rect.left_top(), egui::vec2(2.0, rect.height()));
        painter.rect_filled(bar, 0.0, token::ACCENT.gamma_multiply(0.5));
    } else if response.hovered() {
        painter.rect_filled(rect, 0.0, token::SURFACE_3);
    }
    let x = rect.left() + metric::PANEL_PAD;
    let glyph = egui::Rect::from_min_size(egui::pos2(x, rect.top() + 4.0), egui::Vec2::splat(14.0));
    icon::draw(painter, glyph, Glyph::Grid, if active { token::ACCENT } else { token::TEXT_LO });
    // A long name stops short of the icons rather than running under them.
    let room = if plane.is_some() { rect.right() - 26.0 - ICON_STEP } else { rect.right() };
    let clip = egui::Rect::from_min_max(rect.min, egui::pos2(room, rect.bottom()));
    painter.with_clip_rect(clip).text(
        egui::pos2(x + 20.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        name,
        egui::FontId::proportional(theme::font::VALUE),
        if active { token::TEXT_HI } else { token::TEXT_HI.gamma_multiply(0.85) },
    );
    let Some(plane) = plane else {
        return response.clicked().then_some(Asked::Activate(None));
    };
    // The pencil and the bin at the right edge, placed as the outliner's switches.
    let mut asked = None;
    for (slot, glyph, button_id, what, hover) in [
        (1.0, Glyph::Edit, edit_id(plane), Asked::Edit(plane), "Change it: its face, place, turn and spacing"),
        (0.0, Glyph::Delete, delete_id(plane), Asked::Delete(plane), "Delete it"),
    ] {
        let at = egui::pos2(rect.right() - 22.0 - slot * ICON_STEP, rect.top() + 3.0);
        let icon_rect = egui::Rect::from_min_size(at, egui::Vec2::splat(ICON));
        let button = ui.interact(icon_rect, button_id, egui::Sense::click()).on_hover_text(hover);
        let colour = if button.hovered() { token::TEXT_HI } else { token::TEXT_LO };
        icon::draw(ui.painter(), icon_rect.shrink(1.0), glyph, colour);
        if button.clicked() {
            asked = Some(what);
        }
    }
    if asked.is_none() && response.clicked() {
        asked = Some(Asked::Activate(Some(plane)));
    }
    asked
}

/// The list's last row, which puts a new workplane down: laid out as the rows above, with a plus
/// where they have their glyph, quiet until the pointer is on it. Inert while one is being set up.
fn add_row(app: &App, ui: &mut egui::Ui) -> bool {
    let enabled = app.workplane_tool.is_none();
    let key = app.keymap.shortcut_text(Command::AddWorkplane);
    let hover = match key.is_empty() {
        true => "Put a workplane down on a face of the model".to_string(),
        false => format!("Put a workplane down on a face of the model ({key})"),
    };
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, metric::ROW), egui::Sense::hover());
    let sense = if enabled { egui::Sense::click() } else { egui::Sense::hover() };
    let response = ui.interact(rect, egui::Id::new("workplane-add"), sense).on_hover_text(hover);
    let painter = ui.painter();
    let colour = if !enabled {
        token::TEXT_LO.gamma_multiply(0.45)
    } else if response.hovered() {
        painter.rect_filled(rect, 0.0, token::SURFACE_3);
        token::TEXT_HI
    } else {
        token::TEXT_LO
    };
    // Painted, as the outliner's marks are, rather than trusting the UI font with the glyph.
    let x = rect.left() + metric::PANEL_PAD;
    let centre = egui::pos2(x + 7.0, rect.center().y);
    let stroke = egui::Stroke::new(1.5_f32, colour);
    painter.line_segment([centre - egui::vec2(4.5, 0.0), centre + egui::vec2(4.5, 0.0)], stroke);
    painter.line_segment([centre - egui::vec2(0.0, 4.5), centre + egui::vec2(0.0, 4.5)], stroke);
    painter.text(
        egui::pos2(x + 20.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        "Add a workplane",
        egui::FontId::proportional(theme::font::VALUE),
        colour,
    );
    response.clicked()
}
