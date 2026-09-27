//! What every selected node shows: name, colour and visibility.

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
            // Several selected: say so rather than rename them all to one name.
            ui.add(egui::Label::new(theme::value(format!("{} objects selected", targets.len()))).selectable(false));
        } else if ui.add(egui::TextEdit::singleline(&mut name).desired_width(f32::INFINITY)).changed() {
            app.edit("Rename", Some(&format!("name:{id}")));
            if let Some(node) = app.scene.get_mut(id) {
                node.name = name;
            }
        }
    });

    // Three states: hidden is gone, while a ghost (for positioning a subtraction) stays visible.
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
            // The swatch starts from the node's effective colour, so the picker never jumps to black.
            let inherited = app.scene.effective_colour(id);
            let mut rgb = inherited.map_or_else(|| unpainted_swatch(ui.visuals().dark_mode), |c| c.0);
            let mixed = targets.iter().any(|t| app.scene.effective_colour(*t) != inherited);
            // The picker popup's id, taken before the button is added, as the widget does.
            let picker_popup = ui.auto_id_with("popup");
            if ui.color_edit_button_srgb(&mut rgb).changed() {
                // One undo step for a whole drag through the picker.
                app.paint_from_picker(targets, rgb);
            }
            // One recent swatch per picker visit, added when it closes (issue 85).
            if !egui::Popup::is_id_open(ui.ctx(), picker_popup) {
                app.picker_closed();
            }
            // Enabled only when something has paint of its own to clear.
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

    // The outliner menu's presets, and the colours this document already uses.
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

/// A row of swatches painting the selection on click: one click, no picker.
pub(crate) fn swatch_row(
    app: &mut App,
    ui: &mut egui::Ui,
    label: &str,
    colours: &[(String, egui::Color32)],
    targets: &[NodeId],
) {
    let mut chosen: Option<Colour> = None;
    // The label column is kept even when empty, so swatches line up under the picker.
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

/// The colour an unpainted solid is drawn in, where the picker starts.
pub(crate) fn unpainted_swatch(dark: bool) -> [u8; 3] {
    let solid = crate::render::Palette::for_dark_mode(dark).solid;
    [solid[0], solid[1], solid[2]]
}
