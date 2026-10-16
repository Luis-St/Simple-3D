//! The tool's window and its buttons.

use super::*;
use crate::app::App;
use crate::popup::{PopupEvent, PopupSpec};
use crate::{theme, ui};

/// The tool's window over the viewport, while open (issue 122).
pub(crate) fn show(app: &mut App, ctx: &egui::Context) {
    app.refresh_mirror_tool();
    if app.mirror_tool.is_none() {
        return;
    }
    let spec = PopupSpec { key: KEY, title: "Mirror", width: WIDTH };
    if app.tool_popup(ctx, spec, body, actions) == PopupEvent::Closed {
        app.close_mirror_tool();
    }
}

/// The settings, then one line saying what Apply comes to or what is still to do. The tool is lifted
/// out of the app while drawing, since the fields also borrow app state.
fn body(app: &mut App, ui: &mut egui::Ui) {
    let Some(mut tool) = app.mirror_tool.take() else { return };
    let toggled = super::controls::rows(app, ui, &mut tool);
    app.mirror_tool = Some(tool);
    if toggled {
        app.toggle_mirror_choosing();
    }
    ui.add_space(4.0);
    summary(app, ui);
}

/// What Apply would do, or the next step; only a line that cannot be mirrored across is a warning.
fn summary(app: &App, ui: &mut egui::Ui) {
    let Some(tool) = app.mirror_tool.as_ref() else { return };
    let text = match (tool.choosing, app.mirror_plan()) {
        (true, _) if tool.targets.is_empty() => "Select the objects to mirror in the viewport or the outliner.".into(),
        (true, _) => "Select the objects to mirror, then press Done.".to_string(),
        (false, Err(why)) if tool.ends.iter().all(Option::is_some) => {
            ui.add(egui::Label::new(egui::RichText::new(why).size(theme::font::LABEL).color(theme::token::ACCENT)));
            return;
        }
        (false, Err(why)) => why,
        (false, Ok(plan)) => {
            let count = plan.len();
            let objects = format!("{count} object{}", ui::plural(count));
            match tool.keep {
                true => format!("Mirrored copies of {objects} are made where the outlines show."),
                false => format!("{objects} are mirrored to where the outlines show."),
            }
        }
    };
    ui.add(egui::Label::new(theme::hint(text)).selectable(false));
}

fn actions(app: &mut App, ui: &mut egui::Ui) {
    let ready = app.mirror_tool.as_ref().is_some_and(|tool| !tool.choosing)
        && app.mirror_plan().is_ok_and(|plan| !plan.is_empty());
    if ui::dialog_button(ui, "Apply", ready).clicked() {
        app.apply_mirror();
    }
    crate::app_chrome::cancel_at_left(ui, |ui| {
        if ui::dialog_button(ui, "Cancel", true).clicked() {
            app.close_mirror_tool();
        }
    });
}
