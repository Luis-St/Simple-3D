//! The buttons along the foot.

use crate::app::App;
use crate::theme;
use crate::ui;

/// The window's buttons: name the rule and keep it on the shelf, or put the
/// window away.
///
/// Done rather than Close: nothing here is provisional. Every number typed is
/// already on the pattern, so shutting the window ends the job rather than
/// abandoning it -- there is no cancel to offer.
pub(crate) fn actions(app: &mut App, ui: &mut egui::Ui) {
    // The action row lays itself out from the right, so these are added in the
    // order they are read backwards: Done ends up against the right edge and the
    // name field ends up in front of the button that uses it.
    let named = !simple3d_core::library::sanitise(&app.pattern_tool_name).is_empty();
    if ui::dialog_button(ui, "Done", true).clicked() {
        app.close_pattern_tool();
    }
    // Nothing to keep until there is a rule to keep.
    if ui::dialog_button(ui, "Save", named && app.pattern_tool_started)
        .on_hover_text("Keep this rule for any other project")
        .clicked()
    {
        app.save_current_kind();
    }
    // What is left of the row once the two buttons have taken theirs. The field
    // is the one thing here that can honestly be any width at all, and its own
    // hint says what belongs in it, so it needs no word in front of it.
    //
    // As tall as the buttons beside it: a text field left to its own height sits
    // a few pixels short of them, and three controls on one row at two heights
    // read as one of them having gone wrong.
    let field = ui.available_width().clamp(60.0, 220.0);
    ui.add(
        egui::TextEdit::singleline(&mut app.pattern_tool_name)
            .desired_width(field)
            .min_size(egui::vec2(field, theme::metric::DIALOG_BUTTON))
            .vertical_align(egui::Align::Center)
            .hint_text("Name this rule"),
    );
}
