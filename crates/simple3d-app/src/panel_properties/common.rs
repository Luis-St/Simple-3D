//! What every selected node shows: its name, its colour and its visibility.

use super::*;
use crate::app::App;
use crate::theme::{self, token};
use simple3d_core::scene::{Anchor, Colour, NodeId, Visibility};

pub(crate) fn common(app: &mut App, ui: &mut egui::Ui, targets: &[NodeId]) {
    let Some(&id) = targets.last() else { return };
    let node = app.scene.node(id);
    let is_root = id == app.scene.root();
    let mut name = node.name.clone();
    let visibility = node.visibility();
    let mixed_visibility = targets.iter().any(|t| app.scene.node(*t).visibility() != visibility);
    let mut anchor = node.anchor;
    let many = targets.len() > 1;

    field_row(ui, "Name", "", |ui| {
        if many {
            // Renaming several nodes to one name would make the outliner
            // unreadable, so the field says what is selected instead.
            ui.add(egui::Label::new(theme::value(format!("{} objects selected", targets.len()))).selectable(false));
        } else if ui.add(egui::TextEdit::singleline(&mut name).desired_width(f32::INFINITY)).changed() {
            app.edit("Rename", Some(&format!("name:{id}")));
            if let Some(node) = app.scene.get_mut(id) {
                node.name = name;
            }
        }
    });

    // Three states rather than a checkbox: hidden means *gone*, and a body that
    // has to be seen while it is positioned -- the one about to be subtracted --
    // is a ghost, which is a property of that body and not of the document.
    field_row(
        ui,
        "Shown",
        "Visible: part of the model. Ghost: excluded from the model, drawn as a translucent shell. \
             Hidden: excluded and not drawn at all.",
        |ui| {
            ui.add_enabled_ui(!is_root, |ui| {
                for option in Visibility::ALL {
                    let showing = !mixed_visibility && visibility == option;
                    if theme::choice(ui, showing, option.label()).clicked()
                        && (mixed_visibility || visibility != option)
                    {
                        app.edit("Visibility", None);
                        for target in targets {
                            if let Some(node) = app.scene.get_mut(*target) {
                                node.set_visibility(option);
                            }
                        }
                    }
                }
            });
        },
    );

    field_row(
        ui,
        "Colour",
        "What this node is painted. Painting a group paints everything in it, \
             and the colour follows each surface through a boolean.",
        |ui| {
            // The swatch starts from whatever the node shows now -- its own colour,
            // one inherited from a group above it, or the theme's colour for an
            // unpainted solid -- so opening the picker never jumps to black.
            let inherited = app.scene.effective_colour(id);
            let mut rgb = inherited.map_or_else(|| unpainted_swatch(ui.visuals().dark_mode), |c| c.0);
            let mixed = targets.iter().any(|t| app.scene.effective_colour(*t) != inherited);
            // The picker's own popup, named the way the widget names it: the id
            // is taken before the button is added, which is the moment the
            // widget takes it too. What it is for is below.
            let picker_popup = ui.auto_id_with("popup");
            if ui.color_edit_button_srgb(&mut rgb).changed() {
                // One undo step for a whole drag through the picker, the way a
                // scrubbed field is one step.
                app.paint_from_picker(targets, rgb);
            }
            // And one swatch on the recent row for the whole visit, put there
            // when the picker is put away rather than while it is being dragged
            // through: the shades a drag passes over are not choices (issue 85).
            if !egui::Popup::is_id_open(ui.ctx(), picker_popup) {
                app.picker_closed();
            }
            // Enabled only where clearing would do something: a node that merely
            // inherits a group's colour has none of its own to take away.
            let painted = targets.iter().any(|t| app.scene.subtree_is_painted(*t));
            if ui.add_enabled(painted, egui::Button::new("Clear")).on_hover_text("Back to the theme's colour").clicked()
            {
                app.paint(targets, None, None);
            }
            if mixed {
                ui.add(egui::Label::new(theme::value("mixed")).selectable(false));
            }
        },
    );

    // The same swatches the outliner's menu offers, and the colours this
    // document has actually been painted in: opening the picker to find a
    // colour that is already in the project is the slow way round.
    swatch_row(app, ui, "", &theme::PAINT_PRESETS.map(|(name, colour)| (name.to_string(), colour)), targets);
    let recent: Vec<(String, egui::Color32)> = app
        .custom_recent_colours()
        .iter()
        .map(|c| (format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]), egui::Color32::from_rgb(c[0], c[1], c[2])))
        .collect();
    if !recent.is_empty() {
        swatch_row(app, ui, "Recent", &recent, targets);
    }

    field_row(ui, "Anchor", "Where this node's origin sits. Changing it moves the origin, never the shape.", |ui| {
        let mixed = targets.iter().any(|t| app.scene.node(*t).anchor != anchor);
        for option in Anchor::ALL {
            let showing = !mixed && anchor == option;
            if theme::choice(ui, showing, option.label()).clicked() && (mixed || anchor != option) {
                anchor = option;
                app.edit("Anchor", None);
                for target in targets {
                    if let Some(node) = app.scene.get_mut(*target) {
                        node.anchor = anchor;
                    }
                }
            }
        }
    });
}

/// A row of colour swatches that paints the selection when one is clicked.
///
/// Plain buttons rather than a picker, for the same reason the outliner's menu
/// uses them: one click, and the colour is on the shape.
pub(crate) fn swatch_row(
    app: &mut App,
    ui: &mut egui::Ui,
    label: &str,
    colours: &[(String, egui::Color32)],
    targets: &[NodeId],
) {
    let mut chosen: Option<Colour> = None;
    // The label column is kept even when empty, so the swatches line up under
    // the picker rather than under the labels.
    field_row(ui, label, "", |ui| {
        for (name, colour) in colours {
            let swatch = egui::Button::new("")
                .fill(*colour)
                .stroke(egui::Stroke::new(1.0_f32, token::SURFACE_3))
                .min_size(egui::vec2(16.0, 16.0));
            if ui.add(swatch).on_hover_text(name).clicked() {
                chosen = Some(Colour([colour.r(), colour.g(), colour.b()]));
            }
        }
    });
    if let Some(colour) = chosen {
        app.paint(targets, Some(colour), None);
    }
}

/// The colour an unpainted solid is drawn in, which is where the picker starts.
pub(crate) fn unpainted_swatch(dark: bool) -> [u8; 3] {
    let solid = crate::render::Palette::for_dark_mode(dark).solid;
    [solid[0], solid[1], solid[2]]
}
