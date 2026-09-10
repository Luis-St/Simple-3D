//! The tool's window: the templates, the shelf and the stages, in one column.

use super::*;
use crate::app::App;
use crate::popup::{self, PopupEvent, PopupSpec};
use crate::theme;
use simple3d_core::pattern;
use simple3d_core::scene::NodeId;

/// The id one of the "start from" chips answers to. Named rather than found by
/// the word on it: the properties panel behind the window carries a Kind row
/// with the very same seven words, so a test that went looking for "Linear"
/// found two of them.
pub(crate) fn template_id(kind: u32) -> egui::Id {
    egui::Id::new(("pattern-start-from", kind))
}

/// The id the cross that drops the last stage answers to. Named rather than
/// found by where it sits, so a test can ask the context where it was drawn --
/// the bargain every other grip in the application makes.
pub(crate) fn drop_stage_id() -> egui::Id {
    egui::Id::new("pattern-drop-stage")
}

/// The tool's own window, drawn over the viewport once a frame while it is open
/// (issues 67, 96).
pub(crate) fn show(app: &mut App, ctx: &egui::Context) {
    // The pattern can go while the window is up -- the tool is not modal, and
    // the outliner behind it can delete what it is working on.
    if app.pattern_tool.is_some() && app.pattern_tool_target().is_none() {
        app.close_pattern_tool();
    }
    let Some(id) = app.pattern_tool_target() else { return };
    let bounds = app.viewport_rect;
    // The window names what it is building a rule for. It has to: the selection
    // can move on to something else while it is open.
    let title = format!("Rule for {}", app.scene.node(id).name);
    // Taken out of the map for the duration, so the popup may hold it mutably
    // while its contents hold the application.
    let mut placement = app.popups.remove(KEY).unwrap_or_default();
    let event = popup::show(ctx, bounds, &mut placement, PopupSpec { key: KEY, title: &title, width: WIDTH }, |ui| {
        // Four stages is four columns of fields, which is taller than a short
        // viewport: the body scrolls rather than pushing the buttons off the
        // bottom of the screen where nothing can reach them.
        let (area, restore) = theme::list_scroll_area(ui);
        area.auto_shrink([false, true]).max_height(popup::body_room(bounds)).show(ui, |ui| {
            ui.set_style(restore);
            body(app, ui);
        });
        popup::action_row(ui, |ui| actions(app, ui));
    });
    app.popups.insert(KEY, placement);
    if event == PopupEvent::Closed {
        app.close_pattern_tool();
    }
}

/// The tool's contents: where a rule is started from, the shelf it can be taken
/// off, and the stages themselves.
///
/// The first question is what to start from, and until it is answered it is the
/// only thing in the window (issue 79). A rule built out of stages is a blank
/// form otherwise -- four stage numbers at nothing in particular -- and every
/// rule anyone actually wants is one of the six fixed layouts with something
/// added to it. Answering the question is what fills the form in.
///
/// Answered, the question goes: it was asked, and a row of seven layouts left
/// standing over the stages they produced is a set of buttons that throw the
/// editing away, sitting where the window is read from top to bottom.
pub(crate) fn body(app: &mut App, ui: &mut egui::Ui) {
    let Some(id) = app.pattern_tool_target() else {
        ui.label("The pattern this was opened on is no longer there.");
        return;
    };
    if !app.pattern_tool_started {
        templates(app, ui, id);
        ui.add(
            egui::Label::new(theme::hint(
                "Pick what the rule starts from. Each of the six lays its copies out exactly as that kind does, \
                 with the numbers this pattern is already holding.",
            ))
            .selectable(false),
        );
        return;
    }
    if shelf(app, ui) {
        ui.separator();
    }
    stages(app, ui, id);
}

/// The six fixed kinds and a blank sheet, offered as what a rule starts from
/// (issue 79).
///
/// Every fixed kind is one or two stages spelled out, so any of them can be
/// written back into the stages that say the same thing -- with the numbers the
/// pattern is already holding, not with a stock 20 mm step. Lay a ring of six
/// out with the Circular kind, open this, press Circular, and the rule starts as
/// that ring with three stages left to add to it. Custom is the seventh answer:
/// none of the six, so a blank stage to fill in.
///
/// Drawn only while the question is still open. The options keep a column of
/// their own and wrap inside it, rather than the second line starting back under
/// the word "Start from" -- see
/// [`field_row_boxed`](crate::panel_properties::field_row_boxed).
pub(crate) fn templates(app: &mut App, ui: &mut egui::Ui, id: NodeId) {
    let mut start_from = None;
    crate::panel_properties::field_row_boxed(
        ui,
        "Start from",
        "Begin the rule from what one of the fixed kinds lays out, or from nothing",
        |ui| {
            for (index, name) in pattern::KINDS.iter().enumerate() {
                // Never shown as chosen: this is an action, not a state. The
                // rule *is* the stages once one has been pressed, and a
                // highlighted "Grid" would claim the stages under it are still a
                // grid however far they have since been edited.
                let chip = theme::choice(ui, false, name);
                // It senses nothing; the chip itself answers the pointer.
                ui.interact(chip.rect, template_id(index as u32), egui::Sense::hover());
                let chip = if index == pattern::CUSTOM as usize {
                    chip.on_hover_text("Start from nothing: one stage, to be filled in")
                } else {
                    chip
                };
                if chip.clicked() {
                    start_from = Some(index as u32);
                }
            }
        },
    );
    if let Some(kind) = start_from {
        app.start_rule_from(id, kind);
    }
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
