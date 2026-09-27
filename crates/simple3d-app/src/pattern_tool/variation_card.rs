//! One variation's card inside a stage (issue 79).

use super::*;
use crate::app::App;
use crate::panel_properties::field_row;
use crate::theme::{self};
use simple3d_core::pattern::{self, Variation, Vary, VaryField};
use simple3d_core::primitive::{ParamValue, Params};
use simple3d_core::scene::NodeId;

/// One variation: heading, axis, amount, reach and stepping. Returns whether its cross was pressed.
pub(super) fn variation_card(
    app: &mut App,
    ui: &mut egui::Ui,
    id: NodeId,
    index: usize,
    slot: usize,
    stage: &pattern::Stage,
    params: &Params,
) -> bool {
    let variation = &stage.variations()[slot];
    let key = |field: VaryField| pattern::vary_key(index, slot, field);
    let unit = app.unit();
    let dropped = card_heading(
        ui,
        &variation_name(variation),
        &describe_variation(variation, stage.mode, unit),
        drop_variation_id(index, slot),
        "Take this variation off the stage",
    );
    // Kept when the stage turns, so switching back restores it; the cross removes it.
    if !variation.what.fits(stage.mode) {
        note(ui, "A turn has no gaps between its copies, so this does nothing here.");
        return dropped;
    }
    if variation.what != Vary::Gap {
        axis_choice(app, ui, id, &key(VaryField::Axis), variation);
    }
    // Named for what one step is: more each time, or the same on every copy.
    let (builds_up, repeats) = match variation.what {
        Vary::Shift => ("Shift per step", "Shift by"),
        Vary::Spin => ("Turn per step", "Turn by"),
        Vary::Size => ("Size per step (%)", "Size by (%)"),
        Vary::Gap => ("Gap grows by", "Gap wider by"),
    };
    let amount = if variation.repeats { repeats } else { builds_up };
    field(app, ui, id, &key(VaryField::amount_of(variation.what)), amount, params);
    field(app, ui, id, &key(VaryField::Every), "Every (copies)", params);
    field(app, ui, id, &key(VaryField::Start), "Starting at copy", params);
    field(app, ui, id, &key(VaryField::Steps), "Steps", params);
    // Two alike may stand, but the card says they add up as one.
    let alike = stage
        .variations()
        .iter()
        .enumerate()
        .find(|(other, held)| *other != slot && held.combination() == variation.combination());
    if let Some((other, _)) = alike {
        note(
            ui,
            format!(
                "The same as variation {} on this stage: along the same axis, over the same copies, so the two add up.",
                other + 1
            ),
        );
    }
    dropped
}

/// A variation card's heading: its kind and axis.
fn variation_name(variation: &Variation) -> String {
    match variation.what {
        Vary::Gap => variation.what.name().to_string(),
        what => format!("{} {}", what.name(), pattern::VARY_AXES[variation.axis.min(pattern::ALL_AXES)]),
    }
}

/// The axis choice as plain chips; axis colours are for value fields.
fn axis_choice(app: &mut App, ui: &mut egui::Ui, id: NodeId, key: &str, variation: &Variation) {
    let name = if variation.what == Vary::Spin { "About" } else { "Along" };
    field_row(ui, name, "", |ui| {
        for &axis in variation.what.axes() {
            let chosen = variation.axis == axis;
            if theme::choice(ui, chosen, pattern::VARY_AXES[axis]).clicked() && !chosen {
                app.edit("Set pattern", None);
                if let Some(params) = app.scene.get_mut(id).and_then(|node| node.params_mut()) {
                    params.insert(key.to_string(), ParamValue::Choice(axis as u32));
                }
            }
        }
    });
}

/// What each variation kind is for, on the chip adding it.
pub(super) fn vary_hover(what: Vary) -> &'static str {
    match what {
        Vary::Shift => "Move copies aside where they stand: every other one by half a step staggers rows",
        Vary::Spin => "Turn each copy about its own origin",
        Vary::Size => "Make each copy bigger or smaller than the one before",
        Vary::Gap => "Widen or narrow the gaps along the run",
    }
}
