//! The tool's window: the rule on one side, the picture on the other.

use super::*;
use crate::app::App;
use crate::theme;
use simple3d_core::scene::NodeId;

/// The tool's contents: the rule down the left, and a viewport on what it lays
/// out taking everything else.
///
/// The divider between them is draggable, and the width it is dragged to is the
/// width the stages keep -- through a resize of the window and through the next
/// time the tool is opened, since it is kept with the dock widths. Widening the
/// window therefore widens the picture and nothing else, which is the point: a
/// stage row is a name and a number, and a hundred more pixels only push the
/// two apart, while the viewport is worth more the bigger it is.
///
/// Narrowed past what both need, the stages give width up before the picture
/// disappears, and narrower still the picture is the column that goes: the
/// viewport behind this window is showing the same scene, and the numbers are
/// what the window is open for.
pub(crate) fn body(app: &mut App, ui: &mut egui::Ui) {
    let Some(id) = app.pattern_tool_target() else {
        ui.label("The pattern this was opened on is no longer there.");
        return;
    };
    let room = ui.available_width();
    // What the divider itself costs, so the widest the stages may be still
    // leaves the picture its minimum.
    let rule = ui.spacing().item_spacing.x * 2.0 + 6.0;
    let widest = (room - PREVIEW_MIN - rule).min(STAGES_MAX);

    if widest < STAGES_MIN {
        rule_column(app, ui, id);
        return;
    }
    let wanted = app.settings.pattern_stages_width.clamp(STAGES_MIN, widest);
    let mut width = wanted;
    egui::SidePanel::left("pattern-stages-column")
        .frame(egui::Frame::NONE)
        .resizable(true)
        .default_width(wanted)
        // egui remembers a panel's width itself, so a window narrowed past what
        // the remembered width leaves the picture has to be told the new
        // ceiling -- otherwise the stages keep a width the window no longer has.
        .width_range(STAGES_MIN..=widest)
        .show_inside(ui, |ui| {
            width = ui.available_width();
            // Claim the column's full width up front. egui remembers a panel by
            // the rectangle its *content* filled, and a property row pins its
            // right edge `EDGE_PAD` inside that -- so the column came back eight
            // pixels narrower every frame, and being remembered, kept coming
            // back narrower until it hit its own minimum. The docks have to do
            // exactly this, for exactly this reason.
            ui.expand_to_include_rect(ui.max_rect());
            ui.set_min_width(width);
            rule_column(app, ui, id);
        });
    app.settings.pattern_stages_width = width.clamp(STAGES_MIN, STAGES_MAX);
    preview(app, ui);
}

/// The shelf and the stages, in that order down one column.
pub(crate) fn rule_column(app: &mut App, ui: &mut egui::Ui, id: NodeId) {
    if shelf(app, ui) {
        ui.separator();
    }
    stages(app, ui, id);
}

/// The saved kinds, as one row: pick one to put it on the pattern, and the
/// cross beside it takes that one off the shelf for good.
///
/// It was a column of its own, which on an empty shelf was a paragraph of
/// explanation taking a fifth of the window. Nothing saved, nothing drawn:
/// the row appears with the first kind kept, and until then the button that
/// keeps one is the only thing that needs to be there.
///
/// Returns whether it drew anything, so the caller knows whether to rule a line
/// under it.
pub(crate) fn shelf(app: &mut App, ui: &mut egui::Ui) -> bool {
    if app.pattern_kinds.is_empty() {
        return false;
    }
    let mut apply = None;
    let mut delete = None;
    ui.horizontal(|ui| {
        ui.add(egui::Label::new(theme::header_text("Saved kinds")).selectable(false));
        // The rule on the pattern is the one named in the box, when that name is
        // a saved kind's -- which is what `apply_saved_kind` leaves behind, and
        // what the name field is filled with.
        let on_shelf = app.pattern_kinds.iter().find(|entry| entry.name == app.pattern_tool_name).cloned();
        let shown = match &on_shelf {
            Some(entry) => entry.name.clone(),
            None => "Pick one".to_string(),
        };
        egui::ComboBox::from_id_salt("pattern-shelf")
            .selected_text(theme::value(shown))
            .width((ui.available_width() - 40.0).clamp(80.0, 220.0))
            .show_ui(ui, |ui| {
                for entry in &app.pattern_kinds {
                    let chosen = Some(&entry.name) == on_shelf.as_ref().map(|e| &e.name);
                    if ui.selectable_label(chosen, &entry.name).clicked() {
                        apply = Some(entry.clone());
                    }
                }
            });
        if ui
            .add_enabled(on_shelf.is_some(), egui::Button::new("\u{00d7}"))
            .on_hover_text("Delete the saved kind named here")
            .clicked()
        {
            delete = on_shelf;
        }
    });
    if let Some(entry) = apply {
        app.apply_saved_kind(&entry);
    }
    if let Some(entry) = delete {
        app.delete_saved_kind(&entry);
    }
    true
}
