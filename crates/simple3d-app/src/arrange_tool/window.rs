//! The tool's window and its buttons.

use super::*;
use crate::app::App;
use crate::popup::{PopupEvent, PopupSpec};
use crate::{theme, ui};

/// The tool's window over the viewport, while open (issue 70).
pub(crate) fn show(app: &mut App, ctx: &egui::Context) {
    app.refresh_arrange_tool();
    if app.arrange_tool.is_none() {
        return;
    }
    let spec = PopupSpec { key: KEY, title: "Align and distribute", width: WIDTH };
    if app.tool_popup(ctx, spec, body, actions) == PopupEvent::Closed {
        app.close_arrange_tool();
    }
}

/// The way to arrange, its settings, and what they come to. No picture: the viewport shows the
/// templates. The tool is lifted out of the app while drawing, since the fields also borrow app state.
fn body(app: &mut App, ui: &mut egui::Ui) {
    let Some(mut tool) = app.arrange_tool.take() else {
        ui.label("The objects this was opened on are no longer there.");
        return;
    };
    ui.horizontal_wrapped(|ui| {
        for (mode, hover) in [
            (Arrange::Align, "Line the objects up by their boxes' sides or centres."),
            (Arrange::Distribute, "Space the objects evenly along an axis."),
            (
                Arrange::Path,
                "Spread the objects along a path of body edges or drawn lines, making copies if more are \
                 asked for than are selected.",
            ),
        ] {
            if theme::choice(ui, tool.mode == mode, mode.label()).on_hover_text(hover).clicked() {
                tool.mode = mode;
            }
        }
    });
    ui.add_space(6.0);
    match tool.mode {
        Arrange::Align => align_controls(app, ui, &mut tool),
        Arrange::Distribute => distribute_controls(app, ui, &mut tool),
        Arrange::Path => path_controls(app, ui, &mut tool),
    }
    app.arrange_tool = Some(tool);
    ui.add_space(6.0);
    summary(app, ui);
}

/// What Apply would do, or why it cannot.
fn summary(app: &App, ui: &mut egui::Ui) {
    let text = match app.arrange_plan() {
        Err(why) => {
            ui.add(egui::Label::new(egui::RichText::new(why).size(theme::font::LABEL).color(theme::token::ACCENT)));
            return;
        }
        Ok(placements) => {
            let moving = placements.iter().filter(|p| !p.copy && p.moves()).count();
            let copies = placements.iter().filter(|p| p.copy).count();
            match (moving, copies) {
                (0, 0) => "Everything is already there.".to_string(),
                (_, 0) => format!("{moving} object{} move, to where the outlines show.", ui::plural(moving)),
                _ => {
                    let made = format!(
                        "{copies} cop{} {} made beside the originals, in the same groups",
                        if copies == 1 { "y" } else { "ies" },
                        if copies == 1 { "is" } else { "are" }
                    );
                    match moving {
                        0 => format!("{made}, where the outlines show."),
                        _ => format!("{moving} object{} move and {made}, where the outlines show.", ui::plural(moving)),
                    }
                }
            }
        }
    };
    ui.add(egui::Label::new(theme::hint(text)).selectable(false));
}

fn actions(app: &mut App, ui: &mut egui::Ui) {
    let ready = app.arrange_plan().is_ok_and(|placements| placements.iter().any(Placement::moves));
    if ui::dialog_button(ui, "Apply", ready).clicked() {
        app.apply_arrange();
    }
    crate::app_chrome::cancel_at_left(ui, |ui| {
        if ui::dialog_button(ui, "Cancel", true).clicked() {
            app.close_arrange_tool();
        }
    });
}
