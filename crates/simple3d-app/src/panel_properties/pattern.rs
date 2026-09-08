//! A pattern node's rule.

use super::*;
use crate::app::App;
use crate::theme::{self};
use simple3d_core::primitive::ParamsExt;
use simple3d_core::scene::NodeId;

/// The pattern editor (issue 67): the kind and its numbers, driven from the
/// shared parameter list, plus a line saying what it currently makes.
pub(crate) fn pattern(app: &mut App, ui: &mut egui::Ui, id: NodeId) {
    let params = app.scene.node(id).params().cloned().unwrap_or_default();
    let unit = app.unit();
    let targets = [id];
    // A custom kind is edited in the tool, and nowhere else. Its stages are
    // thirty-odd numbered fields -- "3 Radius per copy", "4 Turn per copy" --
    // and a column of them under the kind row says nothing about the rule they
    // make: which stage repeats which, and what any of it lays down, is what the
    // tool draws beside them. Here they were only a wall to scroll past on the
    // way to the button that opens it.
    let custom = params.int("kind") == simple3d_core::pattern::CUSTOM;
    for param in simple3d_core::pattern::PARAMS {
        if !simple3d_core::pattern::param_visible(param, &params) {
            continue;
        }
        // The kind itself stays: it is how a pattern stops being custom again.
        if custom && param.shown_when == Some(("kind", simple3d_core::pattern::CUSTOM)) {
            continue;
        }
        param_field(app, ui, &targets, id, param, unit, PATTERN_ROW);
    }
    // The seventh kind is one the user writes themselves, and a rule built out
    // of stages is not something to assemble from a column of numbered fields
    // alone -- so where the kind *is* custom, the tool that builds it is one
    // click away (issue 67).
    //
    // Only there. The button used to read "Custom kind..." under every other
    // kind, and clicking it made the pattern custom as a side effect of opening
    // a tool: a second way to choose a kind, sitting under the row that chooses
    // the kind. Becoming custom is the Kind row's to say.
    if custom {
        let mut open_tool = false;
        let mut apply = None;
        field_row(ui, "Rule", "", |ui| {
            // The shelf, where there is one. A kind saved from the tool is meant
            // to be used again, and needing the tool open to reach one -- when
            // reaching it is a single click on a name -- is the tool asking to
            // be visited rather than used.
            //
            // Which one is on this pattern is read the way the tool reads it:
            // applying a kind names the node after it, so a node whose name is a
            // saved kind's is showing that kind. A rule edited afterwards keeps
            // the name, which is why the box says what was picked rather than
            // claiming the numbers still match it.
            if !app.pattern_kinds.is_empty() {
                let node_name = app.scene.node(id).name.clone();
                let picked = app.pattern_kinds.iter().find(|entry| entry.name == node_name).cloned();
                let shown = picked.as_ref().map_or("Pick one", |entry| entry.name.as_str()).to_string();
                let shelf = egui::ComboBox::from_id_salt("pattern-saved-kind")
                    .selected_text(theme::value(shown))
                    .width(fits(ui, 150.0))
                    .show_ui(ui, |ui| {
                        for entry in &app.pattern_kinds {
                            let chosen = picked.as_ref().is_some_and(|p| p.name == entry.name);
                            if ui.selectable_label(chosen, &entry.name).clicked() && !chosen {
                                apply = Some(entry.clone());
                            }
                        }
                    });
                // Named rather than found by where it sits: a combo box carries
                // no label of its own in the accessibility tree -- only the name
                // it happens to be showing, which is the thing under test -- so
                // a test asks the context where it was drawn, the way it asks
                // for a value field's grip. It senses nothing; the box itself
                // answers the pointer.
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
        if let Some(entry) = apply {
            app.apply_saved_kind_to(id, &entry);
        }
        if open_tool {
            app.open_pattern_tool();
        }
    }

    let (wanted, copies) = simple3d_core::pattern::instance_count(&params);
    let children = app.scene.node(id).children.len();
    let note = if children == 0 {
        "Put shapes under this pattern in the outliner -- or add one with it selected -- and it repeats them."
            .to_string()
    } else {
        format!("{copies} copies of {children} shape{}.", if children == 1 { "" } else { "s" })
    };
    ui.add(egui::Label::new(theme::hint(note)).selectable(false));
    // The numbers above are not the only way in: every one of them that places
    // a copy has a handle in the viewport, on the copy it places (issue 67).
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
    // Said out loud rather than silently drawing fewer: a grid multiplies its
    // three counts, so it is easy to ask for a hundred million copies without
    // meaning to, and a pattern that quietly stopped short would just look wrong.
    if wanted > copies {
        ui.add(
            egui::Label::new(theme::hint(format!(
                "Capped at {copies} -- {wanted} copies were asked for, which is more than can be drawn."
            )))
            .selectable(false),
        );
    }
}
