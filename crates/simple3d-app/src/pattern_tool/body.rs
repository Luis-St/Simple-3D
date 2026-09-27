//! The tool's window: what a rule starts from, then the stages, in one column.

use super::*;
use crate::app::App;
use crate::popup::{PopupEvent, PopupSpec};
use crate::theme::{self, token};
use simple3d_core::pattern;
use simple3d_core::scene::NodeId;

/// A "start from" chip's id. Named, since the properties panel has a Kind row with the same words.
pub(crate) fn template_id(kind: u32) -> egui::Id {
    egui::Id::new(("pattern-start-from", kind))
}

/// A ready-made layout's id.
pub(crate) fn preset_id(preset: usize) -> egui::Id {
    egui::Id::new(("pattern-start-preset", preset))
}

/// The id of the cross dropping stage `index`, for tests.
pub(crate) fn drop_stage_id(index: usize) -> egui::Id {
    egui::Id::new(("pattern-drop-stage", index))
}

/// The id of the arrow moving stage `index` up or down.
pub(crate) fn move_stage_id(index: usize, up: bool) -> egui::Id {
    egui::Id::new(("pattern-move-stage", index, up))
}

/// The id of the heading folding stage `index`.
pub(crate) fn fold_stage_id(index: usize) -> egui::Id {
    egui::Id::new(("pattern-fold-stage", index))
}

/// The id of the chip adding a stage doing `mode` (issue 79).
pub(crate) fn add_stage_id(mode: pattern::StageMode) -> egui::Id {
    egui::Id::new(("pattern-add-stage", mode.index()))
}

/// The id of the chip adding a `what` variation to stage `index`.
pub(crate) fn add_variation_id(index: usize, what: pattern::Vary) -> egui::Id {
    egui::Id::new(("pattern-add-variation", index, what.index()))
}

/// The id of the cross removing variation `slot` from stage `index`.
pub(crate) fn drop_variation_id(index: usize, slot: usize) -> egui::Id {
    egui::Id::new(("pattern-drop-variation", index, slot))
}

/// The tool's window over the viewport, while open (issues 67, 96).
pub(crate) fn show(app: &mut App, ctx: &egui::Context) {
    // The tool is not modal, so the pattern may be deleted while it is open.
    if app.pattern_tool.is_some() && app.pattern_tool_target().is_none() {
        app.close_pattern_tool();
    }
    // Found again while drawing stages; a rolled-up window marks none.
    app.pattern_tool_hover = None;
    let Some(id) = app.pattern_tool_target() else { return };
    // Named, since the selection can move on while the window is open.
    let title = format!("Rule for {}", app.scene.node(id).name);
    let spec = PopupSpec { key: KEY, title: &title, width: WIDTH };
    let event = app.tool_popup(ctx, spec, body, actions);
    if event == PopupEvent::Closed {
        app.close_pattern_tool();
    }
}

/// The tool's contents: what to start from, then the stages.
///
/// Until answered, the start question is the only content (issue 79), since a blank stage form
/// is useless. Once answered it goes away, as its buttons would discard the edits; "Start over"
/// brings it back.
pub(crate) fn body(app: &mut App, ui: &mut egui::Ui) {
    let Some(id) = app.pattern_tool_target() else {
        ui.label("The pattern this was opened on is no longer there.");
        return;
    };
    if !app.pattern_tool_started {
        templates(app, ui, id);
        presets(app, ui, id);
        saved_kinds(app, ui, id);
        ui.add(
            egui::Label::new(theme::hint(
                "Pick what the rule starts from. Each of the six lays its copies out exactly as that kind does, \
                 with the numbers this pattern is already holding. The ready made layouts are sized to what the \
                 pattern repeats.",
            ))
            .selectable(false),
        );
        // Reopened over an existing rule, nothing is written yet, so it stays one click away.
        if app.pattern_tool_resumable && ui.button("Back to the current rule").clicked() {
            app.resume_rule();
        }
        return;
    }
    stages(app, ui, id);
    // The scatter is the last card of the builder (issue 79); it used to be a separate window.
    ui.add_space(8.0);
    card(token::SURFACE_0B).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.add(egui::Label::new(theme::header_text("Noise")).selectable(false));
        crate::noise_popup::builder(app, ui, id, crate::panel_properties::PATTERN_TOOL_ROW);
        keep_noise(app, ui, id);
    });
}

/// The six fixed kinds and a blank sheet as starting points (issue 79). Each fixed kind converts
/// to stages using the pattern's current numbers. Drawn only while the question is open; options
/// wrap in their own column ([`field_row_boxed`](crate::panel_properties::field_row_boxed)).
pub(crate) fn templates(app: &mut App, ui: &mut egui::Ui, id: NodeId) {
    let mut start_from = None;
    crate::panel_properties::field_row_boxed(
        ui,
        "Start from",
        "Begin the rule from what one of the fixed kinds lays out, or from nothing",
        |ui| {
            for (index, name) in pattern::KINDS.iter().enumerate() {
                // Never shown as chosen: this is an action, and the stages may since have been edited.
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

/// The ready-made layouts: offset rows, which no fixed kind can express (issue 79).
pub(crate) fn presets(app: &mut App, ui: &mut egui::Ui, id: NodeId) {
    let mut picked = None;
    crate::panel_properties::field_row_boxed(
        ui,
        "Ready made",
        "Layouts none of the fixed kinds can say, sized to what this pattern repeats",
        |ui| {
            for (index, name) in pattern::PRESETS.iter().enumerate() {
                let chip = theme::choice(ui, false, name);
                ui.interact(chip.rect, preset_id(index), egui::Sense::hover());
                if chip.clicked() {
                    picked = Some(index);
                }
            }
        },
    );
    if let Some(preset) = picked {
        app.start_rule_from_preset(id, preset);
    }
}

/// The "Your kinds" dropdown's id. Named, since the properties panel has one showing the same text.
pub(crate) fn saved_start_id() -> egui::Id {
    egui::Id::new("pattern-start-saved")
}

/// The user's saved kinds as a third starting point (issue 79), in the same dropdown as the
/// properties panel's Rule row ([`crate::pattern_tool::items`]). Drawn only once a kind exists.
pub(crate) fn saved_kinds(app: &mut App, ui: &mut egui::Ui, id: NodeId) {
    if app.pattern_kinds.is_empty() {
        return;
    }
    let mut picked = None;
    crate::panel_properties::field_row(ui, "Your kinds", "Rules you have saved, to start from again", |ui| {
        // Nothing chosen yet: the question is what to start from.
        let shelf = egui::ComboBox::from_id_salt("pattern-tool-saved-kind")
            .selected_text(theme::value("Pick one"))
            .width(crate::panel_properties::fits(ui, 150.0))
            .show_ui(ui, |ui| {
                picked = crate::pattern_tool::items(ui, &app.pattern_kinds, None);
            });
        // It senses nothing; the box itself answers the pointer.
        ui.interact(shelf.response.rect, saved_start_id(), egui::Sense::hover());
    });
    match picked {
        Some(ShelfPick::Apply(entry)) => app.apply_saved_kind_to(id, &entry),
        Some(ShelfPick::Delete(entry)) => app.ask_delete_saved_kind(entry),
        None => {}
    }
}

/// Whether Save keeps the pattern's scatter (issue 79); asked only when there is one.
fn keep_noise(app: &mut App, ui: &mut egui::Ui, id: NodeId) {
    if !app.noise_is_set(id) {
        return;
    }
    ui.add_space(6.0);
    ui.checkbox(&mut app.pattern_tool_keep_noise, "Save the noise with the rule")
        .on_hover_text("A saved kind brings this scatter back with it wherever it is used");
}
