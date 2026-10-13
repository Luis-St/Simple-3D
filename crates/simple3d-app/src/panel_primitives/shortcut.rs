//! Giving a primitive a shortcut from its tile (issue 124), without opening the keymap editor.

use crate::app::App;
use crate::theme;
use simple3d_core::keymap::Action;
use simple3d_core::primitive::PrimitiveSpec;

/// The tile's right-click menu: set, change or remove its shortcut.
pub(super) fn tile_menu(app: &mut App, response: &egui::Response, spec: &PrimitiveSpec) {
    let bound = app.keymap.primitive_shortcut_text(spec.type_id);
    response.context_menu(|ui| {
        let set = if bound.is_empty() {
            "Set a shortcut...".to_string()
        } else {
            format!("Change the shortcut ({bound})...")
        };
        if ui.button(set).clicked() {
            app.start_recording(Action::Primitive(spec.type_id.to_string()));
            ui.close();
        }
        if !bound.is_empty() && ui.button("Remove the shortcut").clicked() {
            app.keymap.unbind_primitive(spec.type_id);
            app.persist_keymap();
            ui.close();
        }
    });
}

/// The prompt beside a tile while its shortcut is recorded, or the question when the keys were taken.
pub(super) fn prompt(app: &mut App, ui: &egui::Ui, tile: egui::Rect, spec: &PrimitiveSpec) {
    let target = Action::Primitive(spec.type_id.to_string());
    let recording = app.recording.as_ref() == Some(&target);
    let conflict = app.keymap_conflict.clone().filter(|(wanted, _, _)| *wanted == target);
    if !recording && conflict.is_none() {
        return;
    }
    egui::Area::new(egui::Id::new(("palette-shortcut", spec.type_id)))
        .order(egui::Order::Foreground)
        .fixed_pos(tile.right_top() + egui::vec2(6.0, 0.0))
        .show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_max_width(240.0);
                match &conflict {
                    None => {
                        ui.label(theme::value(format!("Shortcut to add a {}", spec.label.to_lowercase())));
                        // Modifiers and multi-key holds are bindings too (issue 77), so the prompt says so.
                        ui.label(theme::hint("Hold the keys, then let go. Escape cancels."));
                        if ui.button("Cancel").clicked() {
                            app.recording = None;
                        }
                    }
                    // Named, never overwritten silently, as in the keymap editor.
                    Some((target, chord, holder)) => {
                        ui.label(format!("{chord} is currently bound to \"{}\".", holder.label()));
                        ui.label(format!("Reassign it to \"{}\"?", target.label()));
                        ui.horizontal(|ui| {
                            if ui.button("Reassign").clicked() {
                                app.bind_recorded(target.clone(), chord.clone(), true);
                            }
                            if ui.button("Cancel").clicked() {
                                app.keymap_conflict = None;
                            }
                        });
                    }
                }
            });
        });
}
