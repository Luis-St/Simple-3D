//! The selection line, and the strip that confirms a deletion.

use crate::app::App;
use crate::theme::{self, token};

/// How much is selected, if anything.
pub(crate) fn selection_line(app: &mut App, ui: &mut egui::Ui) {
    if app.selection.is_empty() {
        return;
    }
    ui.horizontal(|ui| {
        ui.add_space(theme::metric::PANEL_PAD);
        ui.add(egui::Label::new(theme::hint(format!("{} selected", app.selection.len()))).selectable(false));
    });
    ui.add_space(2.0);
}

/// The inline confirmation for deleting a group.
///
/// It is a strip in the outliner rather than a dialog over the window because
/// the question is about the tree, and the answer is easier to give while
/// still looking at it. The two answers are named for what they do -- neither
/// of them is "OK".
pub(crate) fn confirm_strip(app: &mut App, ui: &mut egui::Ui) {
    if app.pending_delete.is_none() {
        return;
    }
    let names: Vec<String> = app
        .pending_delete
        .as_ref()
        .map(|ids| ids.iter().map(|id| app.scene.node(*id).name.clone()).collect())
        .unwrap_or_default();
    let total = app.pending_delete_count();
    let groups = names.len();
    let children = total - groups;

    egui::Frame::NONE
        .fill(token::DANGER.gamma_multiply(0.16))
        .stroke(egui::Stroke::new(1.0_f32, token::DANGER.gamma_multiply(0.7)))
        .inner_margin(egui::Margin { left: 8, right: 8, top: 6, bottom: 6 })
        .show(ui, |ui| {
            let what = if groups == 1 { format!("\u{201C}{}\u{201D}", names[0]) } else { format!("{groups} groups") };
            ui.add(
                egui::Label::new(theme::value(format!(
                    "Delete {what}? It holds {children} node{}.",
                    if children == 1 { "" } else { "s" }
                )))
                .selectable(false)
                .wrap(),
            );
            ui.horizontal_wrapped(|ui| {
                if ui.button("Delete the children too").on_hover_text("The group and everything inside it").clicked() {
                    app.confirm_delete(false);
                }
                if ui
                    .button("Keep the children")
                    .on_hover_text("The children move up into the group's own place; only the group goes")
                    .clicked()
                {
                    app.confirm_delete(true);
                }
                if ui.button("Cancel").clicked() {
                    app.cancel_delete();
                }
            });
        });
    // Escape is the way out of everything else in this application, so it is
    // the way out of this too.
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.cancel_delete();
    }
}
