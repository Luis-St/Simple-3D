//! The editor's panel: the tool in use and what to do with it, then the tools with their settings
//! (`window_tools.rs`), the view, the selection and what the constraints leave free, the
//! constraint list (in their own files), and the buttons. It fills the right dock
//! while the docks are shown (`dock.rs`), else it floats over the viewport as an in-place popup.

use super::open::finish_blocked;
use super::*;
use crate::app::App;
use crate::popup::{PopupEvent, PopupSpec};
use crate::theme::{self, token};

/// The tool's window over the viewport while open and the docks are hidden, and the popup on what
/// was clicked (issue 74).
pub(crate) fn show(app: &mut App, ctx: &egui::Context) {
    let Some(tool) = app.sketch_tool.as_ref() else { return };
    if !super::dock::in_dock(app) {
        let spec = PopupSpec { key: KEY, title: title(tool), width: WIDTH };
        let body = |app: &mut App, ui: &mut egui::Ui| {
            hint_line(app, ui);
            ui.add_space(4.0);
            body(app, ui);
        };
        if app.tool_popup(ctx, spec, body, actions) == PopupEvent::Closed {
            app.close_sketch_tool();
            return;
        }
    }
    super::inspect::show_inspect(app, ctx);
}

pub(crate) fn title(tool: &SketchTool) -> &'static str {
    if tool.editing.is_some() {
        "Change sketch"
    } else {
        "Sketch"
    }
}

/// A titled part of the panel, rolled up by its header as the properties panel's are.
pub(crate) fn group(ui: &mut egui::Ui, name: &str, contents: impl FnOnce(&mut egui::Ui)) {
    crate::panel_properties::section(ui, name, contents);
}

/// The tool in use by name and key, and what to do with it; before the plane is picked, how to pick it.
pub(crate) fn hint_line(app: &App, ui: &mut egui::Ui) {
    let Some(tool) = app.sketch_tool.as_ref() else { return };
    if tool.frame.is_none() {
        let plane = app.workplane().map_or("the ground".to_string(), |plane| plane.name.clone());
        ui.add(egui::Label::new(theme::value("Pick a plane")).selectable(false));
        let hint = format!(
            "Click one of the orange origin planes or a face of the model to sketch on it, or anywhere else to \
             sketch on {plane}."
        );
        ui.add(egui::Label::new(theme::hint(hint)).selectable(false));
        return;
    }
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(16.0), egui::Sense::hover());
        crate::icon::draw(ui.painter(), rect, tool.draw.glyph(), token::ACCENT);
        ui.add(egui::Label::new(theme::value(tool.draw.named())).selectable(false));
    });
    ui.add(egui::Label::new(theme::hint(tool.draw.hint())).selectable(false));
}

/// The tool is lifted out of the app while drawing, since the fields also borrow app state.
pub(crate) fn body(app: &mut App, ui: &mut egui::Ui) {
    let Some(mut tool) = app.sketch_tool.take() else { return };
    if tool.frame.is_none() {
        app.sketch_tool = Some(tool);
        return;
    }
    super::window_tools::tools(app, ui, &mut tool);
    super::view::group(app, ui, &mut tool);
    group(ui, "Selection", |ui| {
        selection_row(ui, &mut tool);
        freedom(ui, &tool);
    });
    super::window_list::list(app, ui, &mut tool);
    app.sketch_tool = Some(tool);
}

/// The width of one of `columns` buttons sharing a row.
pub(crate) fn button_width(ui: &egui::Ui, columns: usize) -> f32 {
    let gap = ui.spacing().item_spacing.x;
    ((ui.available_width() - gap * (columns as f32 - 1.0)) / columns as f32).floor().max(40.0)
}

/// The narrowest the selection row fits on one line in; below it the count goes above the buttons.
const SELECTION_ROW: f32 = 260.0;

/// How much is selected on the left, or above in a narrow dock; the helper-geometry switch for
/// selected curves and Delete on the right.
fn selection_row(ui: &mut egui::Ui, tool: &mut SketchTool) {
    let curves: Vec<CurveId> =
        tool.selection.iter().filter_map(|p| if let Pick::Curve(id) = p { Some(*id) } else { None }).collect();
    let count = tool.selection.len();
    let said = if count == 0 { "Nothing selected".to_string() } else { format!("{count} selected") };
    let narrow = ui.available_width() < SELECTION_ROW;
    if narrow {
        ui.add(egui::Label::new(theme::hint(said.clone())).selectable(false));
    }
    ui.horizontal(|ui| {
        if !narrow {
            ui.add(egui::Label::new(theme::hint(said)).selectable(false));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add_enabled(count > 0, egui::Button::new("Delete")).on_hover_text("Delete what is selected").clicked()
            {
                tool.delete_selection();
            }
            let helper =
                !curves.is_empty() && curves.iter().all(|&id| tool.sketch.curve(id).is_some_and(|c| c.construction));
            let switch = ui.add_enabled_ui(!curves.is_empty(), |ui| theme::choice(ui, helper, "Helper geometry")).inner;
            if switch
                .on_hover_text("Helper geometry is constrained against and can be an axis, but bounds no region.")
                .clicked()
            {
                tool.change(|sketch| {
                    for &id in &curves {
                        if let Some(curve) = sketch.curve_mut(id) {
                            curve.construction = !helper;
                        }
                    }
                });
            }
        });
    });
}

/// How much the constraints still leave free, behind a dot in the colour it is drawn in.
fn freedom(ui: &mut egui::Ui, tool: &SketchTool) {
    let analysis = &tool.analysis;
    let (colour, text) = if !analysis.redundant.is_empty() {
        (token::DANGER, "Some constraints repeat or fight others; they are marked red.".to_string())
    } else {
        match analysis.dof {
            0 if !tool.sketch.points.is_empty() => (token::ADD, "Fully constrained: nothing can move.".into()),
            0 => (token::TEXT_LO, "Nothing drawn yet.".into()),
            1 => (token::AXIS_Z, "1 degree of freedom left; what can still move is drawn blue.".into()),
            n => (token::AXIS_Z, format!("{n} degrees of freedom left; what can still move is drawn blue.")),
        }
    };
    ui.horizontal_top(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(8.0, 14.0), egui::Sense::hover());
        ui.painter().circle_filled(rect.center(), 3.5, colour);
        ui.add(egui::Label::new(theme::hint(text)).selectable(false).wrap());
    });
}

/// Cancel at the left, then Undo, Redo and Finish, sharing the row evenly.
pub(crate) fn actions(app: &mut App, ui: &mut egui::Ui) {
    let Some(tool) = app.sketch_tool.as_ref() else { return };
    let blocked = finish_blocked(tool);
    let (can_undo, can_redo) = (!tool.undo.is_empty(), !tool.redo.is_empty());
    let width = button_width(ui, 4).min(theme::metric::DIALOG_BUTTON_WIDTH);
    let button = |ui: &mut egui::Ui, label: &str, enabled: bool| {
        ui.add_enabled(enabled, egui::Button::new(label).min_size(egui::vec2(width, theme::metric::DIALOG_BUTTON)))
    };
    let finish = button(ui, "Finish", blocked.is_none());
    let finish = match blocked {
        Some(why) => finish.on_disabled_hover_text(why),
        None => finish,
    };
    if finish.clicked() {
        app.finish_sketch_tool();
        return;
    }
    if button(ui, "Redo", can_redo).clicked() {
        if let Some(tool) = app.sketch_tool.as_mut() {
            tool.redo_step();
        }
    }
    if button(ui, "Undo", can_undo).clicked() {
        if let Some(tool) = app.sketch_tool.as_mut() {
            tool.undo_step();
        }
    }
    crate::app_chrome::cancel_at_left(ui, |ui| {
        if button(ui, "Cancel", true).clicked() {
            app.close_sketch_tool();
        }
    });
}
