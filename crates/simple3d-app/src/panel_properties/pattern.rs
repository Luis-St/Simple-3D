//! A pattern node's rule.

use super::*;
use crate::app::App;
use crate::theme::{self};
use simple3d_core::primitive::ParamsExt;
use simple3d_core::scene::NodeId;

/// The pattern editor (issue 67): the kind and its numbers, and a line on what it makes.
pub(crate) fn pattern(app: &mut App, ui: &mut egui::Ui, id: NodeId) {
    let params = app.scene.node(id).params().cloned().unwrap_or_default();
    let unit = app.unit();
    let targets = [id];
    // A custom kind's stage fields are edited only in the tool, which shows what they make; here
    // they were just a wall to scroll past.
    let custom = params.int("kind") == simple3d_core::pattern::CUSTOM;
    let noise_keys = simple3d_core::pattern::noise_keys();
    for param in simple3d_core::pattern::PARAMS {
        if !simple3d_core::pattern::param_visible(param, &params) {
            continue;
        }
        // The kind row stays: it is how a pattern stops being custom.
        if custom && param.shown_when == Some(("kind", simple3d_core::pattern::CUSTOM)) {
            continue;
        }
        // The scatter is its own group, under the rule.
        if noise_keys.contains(&param.key) {
            continue;
        }
        param_field(app, ui, &targets, id, param, unit, PATTERN_ROW);
    }
    // Open the tool from a fixed kind (issue 79) on the start question, writing nothing until
    // answered, so the Kind row stays the only place a kind is chosen. The old button made the
    // pattern custom as a side effect.
    if !custom {
        let mut open_tool = false;
        field_row(ui, "Rule", "", |ui| {
            if ui
                .button("Customise")
                .on_hover_text("Build a rule of your own out of stages, starting from this layout or another")
                .clicked()
            {
                open_tool = true;
            }
        });
        if open_tool {
            app.open_pattern_tool();
        }
    }
    if custom {
        let mut open_tool = false;
        let mut apply = None;
        field_row(ui, "Rule", "", |ui| {
            // The saved kinds, reachable without opening the tool. The current one is recognised by the
            // node's name, which applying a kind sets.
            if !app.pattern_kinds.is_empty() {
                let node_name = app.scene.node(id).name.clone();
                let picked = app.pattern_kinds.iter().find(|entry| entry.name == node_name).cloned();
                let shown = picked.as_ref().map_or("Pick one", |entry| entry.name.as_str()).to_string();
                let shelf = egui::ComboBox::from_id_salt("pattern-saved-kind")
                    .selected_text(theme::value(shown))
                    .width(fits(ui, 150.0))
                    .show_ui(ui, |ui| {
                        // The same rows as the tool's shelf, with delete crosses, so they behave the same.
                        apply = crate::pattern_tool::items(
                            ui,
                            &app.pattern_kinds,
                            picked.as_ref().map(|p| p.name.as_str()),
                        );
                    });
                // A named id so tests can find the combo box, which has no label of its own; the box answers
                // the pointer.
                ui.interact(shelf.response.rect, saved_kind_id(), egui::Sense::hover());
            }
            if ui
                .button("Edit kind")
                .on_hover_text("Build this pattern's rule out of stages, and keep it for other projects")
                .clicked()
            {
                open_tool = true;
            }
        });
        match apply {
            Some(crate::pattern_tool::ShelfPick::Apply(entry)) => app.apply_saved_kind_to(id, &entry),
            Some(crate::pattern_tool::ShelfPick::Delete(entry)) => app.ask_delete_saved_kind(entry),
            None => {}
        }
        if open_tool {
            app.open_pattern_tool();
        }
    }

    noise(app, ui, id, &params);

    let (wanted, copies) = simple3d_core::pattern::instance_count(&params);
    let children = app.scene.node(id).children.len();
    let note = if children == 0 {
        "Put shapes under this pattern in the outliner -- or add one with it selected -- and it repeats them."
            .to_string()
    } else {
        // Avoid "1 copies" for a blank custom rule.
        format!(
            "{copies} cop{} of {children} shape{}.",
            if copies == 1 { "y" } else { "ies" },
            if children == 1 { "" } else { "s" }
        )
    };
    ui.add(egui::Label::new(theme::hint(note)).selectable(false));
    // Every number that places a copy also has a viewport handle on that copy (issue 67).
    let grips = app.pattern_grips(id).len();
    if grips > 0 {
        ui.add(
            egui::Label::new(theme::hint(format!(
                "{grips} handle{} in the viewport lay{} this out by eye.",
                if grips == 1 { "" } else { "s" },
                if grips == 1 { "s" } else { "" }
            )))
            .selectable(false),
        );
    }
    // Said out loud when the cap cuts copies, since a grid easily asks for millions.
    if wanted > copies {
        ui.add(
            egui::Label::new(theme::hint(format!(
                "Capped at {copies} -- {wanted} copies were asked for, which is more than can be drawn."
            )))
            .selectable(false),
        );
    }
}

/// How far each copy may wander (issue 79), offered for every kind. One summary line here; the
/// numbers live in their own window ([`crate::noise_popup`]) so the panel does not reflow.
pub(crate) fn noise(app: &mut App, ui: &mut egui::Ui, id: NodeId, params: &simple3d_core::primitive::Params) {
    let unit = app.unit();
    let scatter = simple3d_core::pattern::Noise::of(params);
    let summary = if scatter.wanted() {
        let mut parts = vec![
            format!(
                "up to {} {}",
                simple3d_core::unit::format_length(scatter.offset.x.max(scatter.offset.y).max(scatter.offset.z), unit),
                unit.suffix()
            ),
            format!(
                "{}\u{00B0}",
                simple3d_core::unit::format_number(scatter.turn.x.max(scatter.turn.y).max(scatter.turn.z), 1)
            ),
        ];
        if scatter.scale > 1e-9 {
            parts.push(format!("\u{00B1}{} %", simple3d_core::unit::format_number(scatter.scale * 100.0, 0)));
        }
        parts.join(", ")
    } else {
        "none".to_string()
    };
    // A button that opens the window, not a toggle, which read as switching the scatter on.
    let mut open_window = false;
    field_row(ui, "Noise", "Nudge and turn every copy a little off where the rule puts it", |ui| {
        if ui.button("Configure").on_hover_text("Open the scatter's own window over the viewport").clicked() {
            open_window = true;
        }
        ui.add(egui::Label::new(theme::hint(summary)).selectable(false));
    });
    if open_window {
        app.noise_popup = Some(id);
    }
}
