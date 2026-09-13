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
    let noise_keys = simple3d_core::pattern::noise_keys();
    for param in simple3d_core::pattern::PARAMS {
        if !simple3d_core::pattern::param_visible(param, &params) {
            continue;
        }
        // The kind itself stays: it is how a pattern stops being custom again.
        if custom && param.shown_when == Some(("kind", simple3d_core::pattern::CUSTOM)) {
            continue;
        }
        // The scatter is a group of its own, under the rule rather than in it.
        if noise_keys.contains(&param.key) {
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
    // A fixed kind is a rule too, and the tool is where a rule of one's own is
    // built from it (issue 79). The button opens the tool on the question of
    // what to start from -- this kind among the answers -- and writes nothing
    // until that is answered, so the Kind row stays the one place a kind is
    // chosen. The button that used to sit here made the pattern custom as a
    // side effect of opening a tool; this one does not.
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
                        // The same rows the tool's own shelf draws, each with the
                        // cross that deletes that one kind: it is the same list,
                        // so it had better behave the same way.
                        apply = crate::pattern_tool::items(
                            ui,
                            &app.pattern_kinds,
                            picked.as_ref().map(|p| p.name.as_str()),
                        );
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
        // Both halves count, and a blank custom rule makes exactly one copy --
        // where "1 copies" is the line drawing attention to itself rather than
        // to the rule it is reporting on.
        format!(
            "{copies} cop{} of {children} shape{}.",
            if copies == 1 { "y" } else { "ies" },
            if children == 1 { "" } else { "s" }
        )
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

/// How far each copy may wander off where the rule puts it (issue 79).
///
/// Under the rule rather than in it, and offered whatever the kind is: a run of
/// planks wants a little randomness as much as a rule built out of stages does,
/// and an exact pattern is the thing being departed from rather than a seventh
/// kind of one.
///
/// One line here, and the six numbers themselves in a window of their own (see
/// [`crate::noise_popup`]). The line says whether there is any scatter and how
/// much, which is what the panel is for; unrolling the fields into the panel
/// pushed the numbers below them down the page and took the scroll position
/// with them, to edit something only the viewport can show the effect of.
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
    // A button, not a chip that flips: what it does is open a window, and that
    // window closes by its own cross and its own Done, like every other in-place
    // popup here. Drawn as a toggle it read as a switch that turns the scatter
    // on -- which is what the numbers in the window do -- and it said "Hide"
    // while the thing it would hide was a window the user could already see the
    // close cross on. Named for that, too: "Set" is what the numbers inside do,
    // and this only opens the place they are set in.
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
