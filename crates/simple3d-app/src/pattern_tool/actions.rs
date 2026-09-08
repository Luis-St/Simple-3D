//! The buttons along the bottom.

use crate::app::{App, Modal};
use crate::ui;

/// The dialog's buttons: name the rule and keep it, or close.
pub(crate) fn actions(app: &mut App, ui: &mut egui::Ui) {
    // The action row lays itself out from the right, so these are added in the
    // order they are read backwards: Close ends up against the right edge and
    // the name field's label ends up in front of the field it names.
    let named = !simple3d_core::library::sanitise(&app.pattern_tool_name).is_empty();
    if ui::dialog_button(ui, "Close", true).clicked() {
        app.modal = Modal::None;
    }
    if ui::dialog_button(ui, "Save kind", named).clicked() {
        app.save_current_kind();
    }
    // What is left of the row once the two buttons have taken theirs, less the
    // width of the word in front of the field. A fixed 180 was wider than a
    // narrow window has to give, and the name field is the one thing here that
    // can honestly be any width at all.
    let field = (ui.available_width() - 52.0).clamp(60.0, 220.0);
    ui.add(egui::TextEdit::singleline(&mut app.pattern_tool_name).desired_width(field).hint_text("Bolt ring"));
    // The word in front of the field is the first thing to go when the row runs
    // out: the field's own hint already says what belongs in it, and a label half
    // off the edge of the window says less than no label at all.
    if ui.available_width() >= 44.0 {
        ui.add(egui::Label::new("Name").selectable(false));
    }
}
