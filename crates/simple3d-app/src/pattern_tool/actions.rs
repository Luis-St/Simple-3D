//! The buttons along the foot.

use crate::app::App;
use crate::theme;
use crate::ui;

/// The window's buttons: save the rule under a name, or Done (nothing is provisional, so no cancel).
pub(crate) fn actions(app: &mut App, ui: &mut egui::Ui) {
    // Laid out from the right, so added in reverse: Done at the edge, the name field before Save.
    let named = !simple3d_core::library::sanitise(&app.pattern_tool_name).is_empty();
    if ui::dialog_button(ui, "Done", true).clicked() {
        app.close_pattern_tool();
    }
    // Nothing to save until there is a rule.
    if ui::dialog_button(ui, "Save", named && app.pattern_tool_started)
        .on_hover_text("Keep this rule for any other project")
        .clicked()
    {
        app.save_current_kind();
    }
    // The rest of the row goes to the name field (its hint says what it is), as tall as the buttons.
    let field = ui.available_width().clamp(60.0, 220.0);
    ui.add(
        egui::TextEdit::singleline(&mut app.pattern_tool_name)
            .desired_width(field)
            .min_size(egui::vec2(field, theme::metric::DIALOG_BUTTON))
            .vertical_align(egui::Align::Center)
            .hint_text("Name this rule"),
    );
}
