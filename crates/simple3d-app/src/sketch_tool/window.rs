//! The editor's window: the drawing tools, the constraint buttons and what the constraints leave
//! free, then the constraint list and the feature (in their own files), and the buttons.

use super::constrain::Rule;
use super::open::finish_blocked;
use super::*;
use crate::app::{App, Status};
use crate::popup::{PopupEvent, PopupSpec};
use crate::{theme, ui};

/// The tool's window over the viewport, while open (issue 74).
pub(crate) fn show(app: &mut App, ctx: &egui::Context) {
    let Some(tool) = app.sketch_tool.as_ref() else { return };
    let title = if tool.editing.is_some() { "Change sketch" } else { "Sketch" };
    let spec = PopupSpec { key: KEY, title, width: WIDTH };
    if app.tool_popup(ctx, spec, body, actions) == PopupEvent::Closed {
        app.close_sketch_tool();
    }
}

/// A section heading inside the window.
pub(crate) fn heading(ui: &mut egui::Ui, text: &str) {
    ui.add_space(6.0);
    ui.add(egui::Label::new(theme::header_text(text)).selectable(false));
    ui.add_space(2.0);
}

/// The tool is lifted out of the app while drawing, since the fields also borrow app state.
fn body(app: &mut App, ui: &mut egui::Ui) {
    let Some(mut tool) = app.sketch_tool.take() else { return };
    if tool.frame.is_none() {
        let plane = app.workplane().map_or("the ground".to_string(), |plane| plane.name.clone());
        let hint = format!("Click a face of the model to sketch on it, or anywhere else to sketch on {plane}.");
        ui.add(egui::Label::new(theme::hint(hint)).selectable(false));
        app.sketch_tool = Some(tool);
        return;
    }
    ui.add(egui::Label::new(theme::hint(tool.draw.hint())).selectable(false));
    heading(ui, "Draw");
    tool_grid(ui, &mut tool, &Draw::DRAWING, 4);
    heading(ui, "Pick");
    tool_grid(ui, &mut tool, &[Draw::Regions, Draw::Axis, Draw::CopyEdges], 3);
    ui.checkbox(&mut tool.copy_as_helper, "Copy edges as helper geometry")
        .on_hover_text("Copied edges are fixed either way; as helper geometry they bound no region.");
    heading(ui, "Constrain");
    if let Some(message) = rule_grid(ui, &mut tool) {
        app.status = message;
    }
    selection_row(ui, &mut tool);
    ui.add_space(4.0);
    ui.add(egui::Label::new(theme::hint(freedom(&tool))).selectable(false));
    super::window_list::list(app, ui, &mut tool);
    super::window_feature::feature(app, ui, &mut tool);
    app.sketch_tool = Some(tool);
}

/// Buttons picking a tool, `columns` to a row, the one in use lit.
fn tool_grid(ui: &mut egui::Ui, tool: &mut SketchTool, tools: &[Draw], columns: usize) {
    let width = (WIDTH - 24.0) / columns as f32 - 4.0;
    for row in tools.chunks(columns) {
        ui.horizontal(|ui| {
            for &draw in row {
                let button = egui::Button::selectable(tool.draw == draw, draw.label()).min_size(egui::vec2(width, 0.0));
                if ui.add(button).on_hover_text(draw.hint()).clicked() {
                    tool.set_draw(draw);
                }
            }
        });
    }
}

/// The constraint buttons, dimmed with what to select when the selection does not fit. The status
/// line to show after a click, if any.
fn rule_grid(ui: &mut egui::Ui, tool: &mut SketchTool) -> Option<Status> {
    let width = (WIDTH - 24.0) / 3.0 - 4.0;
    let mut message = None;
    for row in Rule::ALL.chunks(3) {
        ui.horizontal(|ui| {
            for &rule in row {
                let fits = tool.kinds_for(rule).is_some();
                let button = egui::Button::new(rule.label()).min_size(egui::vec2(width, 0.0));
                let response = ui
                    .add_enabled(fits, button)
                    .on_disabled_hover_text(format!("Select {}", rule.needs().to_lowercase()));
                if response.clicked() {
                    message = Some(match tool.apply(rule) {
                        Ok(_) => Status::Info(format!("Added {}", rule.label().to_lowercase())),
                        Err(why) => Status::Warning(why),
                    });
                }
            }
        });
    }
    message
}

/// What is selected, with the helper-geometry switch for selected curves and Delete.
fn selection_row(ui: &mut egui::Ui, tool: &mut SketchTool) {
    let curves: Vec<CurveId> =
        tool.selection.iter().filter_map(|p| if let Pick::Curve(id) = p { Some(*id) } else { None }).collect();
    ui.horizontal(|ui| {
        let helper =
            !curves.is_empty() && curves.iter().all(|&id| tool.sketch.curve(id).is_some_and(|c| c.construction));
        if ui
            .add_enabled(!curves.is_empty(), egui::Button::selectable(helper, "Helper geometry"))
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
        if ui.add_enabled(!tool.selection.is_empty(), egui::Button::new("Delete")).clicked() {
            tool.delete_selection();
        }
        let count = tool.selection.len();
        if count > 0 {
            ui.add(egui::Label::new(theme::hint(format!("{count} selected"))).selectable(false));
        }
    });
}

/// How much the constraints still leave free, in a sentence.
fn freedom(tool: &SketchTool) -> String {
    let analysis = &tool.analysis;
    if !analysis.redundant.is_empty() {
        return "Some constraints repeat or fight others; they are marked red.".into();
    }
    match analysis.dof {
        0 if !tool.sketch.points.is_empty() => "Fully constrained: nothing can move.".into(),
        0 => "Nothing drawn yet.".into(),
        1 => "1 degree of freedom left; what can still move is drawn blue.".into(),
        n => format!("{n} degrees of freedom left; what can still move is drawn blue."),
    }
}

fn actions(app: &mut App, ui: &mut egui::Ui) {
    let Some(tool) = app.sketch_tool.as_ref() else { return };
    let blocked = finish_blocked(tool);
    let (can_undo, can_redo) = (!tool.undo.is_empty(), !tool.redo.is_empty());
    let finish = ui::dialog_button(ui, "Finish", blocked.is_none());
    let finish = match blocked {
        Some(why) => finish.on_disabled_hover_text(why),
        None => finish,
    };
    if finish.clicked() {
        app.finish_sketch_tool();
        return;
    }
    if ui::dialog_button(ui, "Redo", can_redo).clicked() {
        if let Some(tool) = app.sketch_tool.as_mut() {
            tool.redo_step();
        }
    }
    if ui::dialog_button(ui, "Undo", can_undo).clicked() {
        if let Some(tool) = app.sketch_tool.as_mut() {
            tool.undo_step();
        }
    }
    crate::app_chrome::cancel_at_left(ui, |ui| {
        if ui::dialog_button(ui, "Cancel", true).clicked() {
            app.close_sketch_tool();
        }
    });
}
