//! The tool's window: what a rule starts from, and then the stages, in one
//! column.

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

/// The id one of the ready-made layouts answers to.
pub(crate) fn preset_id(preset: usize) -> egui::Id {
    egui::Id::new(("pattern-start-preset", preset))
}

/// The id of the cross that drops stage `index`. Named rather than found by
/// where it sits, so a test can ask the context where it was drawn -- the
/// bargain every other grip in the application makes.
pub(crate) fn drop_stage_id(index: usize) -> egui::Id {
    egui::Id::new(("pattern-drop-stage", index))
}

/// The id of the arrow that moves stage `index` up the stack, or down it.
pub(crate) fn move_stage_id(index: usize, up: bool) -> egui::Id {
    egui::Id::new(("pattern-move-stage", index, up))
}

/// The id of the heading that folds stage `index` up, or opens it again.
pub(crate) fn fold_stage_id(index: usize) -> egui::Id {
    egui::Id::new(("pattern-fold-stage", index))
}

/// The id of the line that opens or shuts stage `index`'s "Vary" section.
pub(crate) fn vary_id(index: usize) -> egui::Id {
    egui::Id::new(("pattern-vary", index))
}

/// The tool's own window, drawn over the viewport once a frame while it is open
/// (issues 67, 96).
pub(crate) fn show(app: &mut App, ctx: &egui::Context) {
    // The pattern can go while the window is up -- the tool is not modal, and
    // the outliner behind it can delete what it is working on.
    if app.pattern_tool.is_some() && app.pattern_tool_target().is_none() {
        app.close_pattern_tool();
    }
    // Whichever stage the pointer is over is found again as the stages are
    // drawn; a window rolled up to its bar draws none, and marks none.
    app.pattern_tool_hover = None;
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

/// The tool's contents: where a rule is started from, and then the stages
/// themselves.
///
/// The first question is what to start from, and until it is answered it is the
/// only thing in the window (issue 79). A rule built out of stages is a blank
/// form otherwise -- four stage numbers at nothing in particular -- and every
/// rule anyone actually wants is one of the fixed layouts, a ready-made one or a
/// kind of their own, with something added to it. Answering the question is
/// what fills the form in.
///
/// Answered, the question goes: a row of layouts left standing over the stages
/// they produced is a set of buttons that throw the editing away, sitting where
/// the window is read from top to bottom. "Start over", beside the stages, is
/// what brings it back.
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
                 with the numbers this pattern is already holding; the ready-made layouts are sized to what the \
                 pattern repeats.",
            ))
            .selectable(false),
        );
        // Brought back over a rule that is still on the pattern, the question
        // is not the only way on: nothing has been written, and the rule it was
        // asked over is one click away rather than lost to having asked.
        if app.pattern_tool_resumable && ui.button("Back to the current rule").clicked() {
            app.resume_rule();
        }
        return;
    }
    stages(app, ui, id);
    keep_noise(app, ui, id);
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

/// The layouts that ship ready made: rows offset against each other, which no
/// fixed kind can say (issue 79).
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

/// The kinds the user has saved, as the third answer to the same question
/// (issue 79): a rule of their own is as much a thing to start from as a fixed
/// kind or a ready-made one, and the shelf used to be a dropdown of its own
/// beside the stages -- a second place to start a rule from, below the rule it
/// would replace.
///
/// Each carries the cross that asks to delete it, on the chip's own row, so
/// deleting one never means picking it first.
///
/// Nothing saved, nothing drawn: the row appears with the first kind kept.
pub(crate) fn saved_kinds(app: &mut App, ui: &mut egui::Ui, id: NodeId) {
    if app.pattern_kinds.is_empty() {
        return;
    }
    let mut picked = None;
    crate::panel_properties::field_row_boxed(ui, "Your kinds", "Rules you have saved, to start from again", |ui| {
        // What a chip may take of the column once its cross has had its share.
        let room = (ui.max_rect().width() - SAVED_CROSS).max(24.0);
        for entry in &app.pattern_kinds {
            // The name and its cross are one unit on the row, so a wrap never
            // leaves a cross at the start of a line under someone else's name.
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                let shown = fitted(ui, &entry.name, room);
                let chip = theme::choice(ui, false, &shown);
                let chip = if shown == entry.name { chip } else { chip.on_hover_text(&entry.name) };
                if chip.clicked() {
                    picked = Some(ShelfPick::Apply(entry.clone()));
                }
                if ui
                    .add(egui::Button::new(theme::hint("\u{00d7}")).small())
                    .on_hover_text(format!("Delete \u{201C}{}\u{201D} from the shelf", entry.name))
                    .clicked()
                {
                    picked = Some(ShelfPick::Delete(entry.clone()));
                }
            });
        }
    });
    match picked {
        Some(ShelfPick::Apply(entry)) => app.apply_saved_kind_to(id, &entry),
        Some(ShelfPick::Delete(entry)) => app.ask_delete_saved_kind(entry),
        None => {}
    }
}

/// The width a saved kind's cross and the gap before it take out of its row.
const SAVED_CROSS: f32 = 24.0;

/// A saved kind's name, cut short with an ellipsis where the whole of it would
/// not fit `room`.
///
/// A kind is named by whoever saved it, and a chip is one line that cannot
/// wrap: a long name ran the chip, and the cross after it, off the side of the
/// window. The whole name is still what the chip's tooltip says.
fn fitted(ui: &egui::Ui, name: &str, room: f32) -> String {
    let font = egui::TextStyle::Button.resolve(ui.style());
    let padding = ui.spacing().button_padding.x * 2.0 + 2.0;
    let width = |text: &str| {
        ui.painter().layout_no_wrap(text.to_string(), font.clone(), egui::Color32::WHITE).size().x + padding
    };
    if width(name) <= room {
        return name.to_string();
    }
    let chars: Vec<char> = name.chars().collect();
    (1..chars.len())
        .rev()
        .map(|keep| chars[..keep].iter().collect::<String>().trim_end().to_string() + "\u{2026}")
        .find(|short| width(short) <= room)
        .unwrap_or_else(|| "\u{2026}".to_string())
}

/// Whether Save keeps the pattern's scatter with the rule (issue 79). Only
/// asked where there is a scatter to keep.
fn keep_noise(app: &mut App, ui: &mut egui::Ui, id: NodeId) {
    if !app.noise_is_set(id) {
        return;
    }
    ui.add_space(6.0);
    ui.checkbox(&mut app.pattern_tool_keep_noise, "Save the noise with the rule")
        .on_hover_text("A saved kind brings this scatter back with it wherever it is used");
}
