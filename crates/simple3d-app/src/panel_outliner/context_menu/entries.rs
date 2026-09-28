//! The context menu's building blocks: dividers, command entries, the Add submenu and the paint swatches.

use simple3d_core::keymap::{Command, Keymap};
use simple3d_core::scene::{Colour, ComponentId, GroupOp};

/// Dividers between the menu's blocks, drawn only once something follows, so there are never
/// two in a row or one at the bottom.
pub(super) struct Blocks {
    pub(super) started: bool,
    pub(super) ruled: bool,
}

impl Blocks {
    /// A block ends here; the next entry starts a new one.
    pub(super) fn rule(&mut self) {
        self.ruled = true;
    }

    pub(super) fn entry(&mut self, ui: &mut egui::Ui) {
        if self.ruled && self.started {
            ui.separator();
        }
        self.ruled = false;
        self.started = true;
    }
}

/// One command in the menu, omitted where it would do nothing. The choice is collected rather
/// than run: labelling borrows the keymap, running needs the whole application.
pub(super) fn item(
    ui: &mut egui::Ui,
    blocks: &mut Blocks,
    keymap: &Keymap,
    chosen: &mut Option<Command>,
    command: Command,
    available: bool,
) {
    if !available {
        return;
    }
    blocks.entry(ui);
    if crate::ui::menu_entry(ui, &crate::ui::menu_label(keymap, command), true).clicked() {
        *chosen = Some(command);
        ui.close();
    }
}

/// The "Add" submenu: groups, patterns, the project's components and the primitive palette.
pub(super) fn add_menu(
    ui: &mut egui::Ui,
    placeable: &[(ComponentId, String, Option<String>)],
    add: &mut Option<(Option<&'static str>, GroupOp)>,
    add_pattern: &mut bool,
    add_custom_pattern: &mut bool,
    place_component: &mut Option<ComponentId>,
) {
    for op in GroupOp::ALL {
        if ui.button(format!("{} group", op.label())).clicked() {
            *add = Some((None, op));
            ui.close();
        }
    }
    // Separates combining containers (groups) from repeating ones (patterns).
    ui.separator();
    // An empty pattern (issue 67); wrapping the selection is further down.
    if ui.button("Pattern").on_hover_text("An empty pattern, for shapes to be put into it and repeated").clicked() {
        *add_pattern = true;
        ui.close();
    }
    // The custom-pattern tool (issue 67), next to "Pattern" so it can be found.
    if ui
        .button("Custom pattern")
        .on_hover_text("Build a repetition rule out of stages, and keep it for other projects")
        .clicked()
    {
        *add_custom_pattern = true;
        ui.close();
    }
    ui.separator();
    // The project's components as their own category (issue 113), once there is one to place.
    if !placeable.is_empty() {
        ui.menu_button("Components", |ui| {
            for (component, name, refusal) in placeable {
                let button = ui.add_enabled(refusal.is_none(), egui::Button::new(name));
                if let Some(why) = refusal {
                    button.on_disabled_hover_text(why);
                } else if button.clicked() {
                    *place_component = Some(*component);
                    ui.close();
                }
            }
        });
        ui.separator();
    }
    for category in simple3d_core::primitive::categories() {
        ui.menu_button(category, |ui| {
            for spec in simple3d_core::primitive::REGISTRY.iter().filter(|s| s.category == category) {
                if ui.button(spec.label).clicked() {
                    *add = Some((Some(spec.type_id), GroupOp::Union));
                    ui.close();
                }
            }
        });
    }
}

/// The paint swatches: the presets, then the user's custom recent colours.
pub(super) fn colour_rows(ui: &mut egui::Ui, recent: Vec<[u8; 3]>, paint: &mut Option<Option<Colour>>) {
    ui.horizontal(|ui| {
        for (name, preset) in crate::theme::PAINT_PRESETS {
            let swatch = egui::Button::new("")
                .fill(preset)
                .stroke(egui::Stroke::new(1.0_f32, crate::theme::token::SURFACE_3))
                .min_size(egui::vec2(16.0, 16.0));
            if ui.add(swatch).on_hover_text(name).clicked() {
                *paint = Some(Some(Colour([preset.r(), preset.g(), preset.b()])));
                ui.close();
            }
        }
    });
    // The user's custom recent colours; presets are already in the row above.
    if !recent.is_empty() {
        ui.horizontal(|ui| {
            for colour in recent {
                let swatch = egui::Button::new("")
                    .fill(egui::Color32::from_rgb(colour[0], colour[1], colour[2]))
                    .stroke(egui::Stroke::new(1.0_f32, crate::theme::token::SURFACE_3))
                    .min_size(egui::vec2(16.0, 16.0));
                let name = format!("#{:02x}{:02x}{:02x}", colour[0], colour[1], colour[2]);
                if ui.add(swatch).on_hover_text(name).clicked() {
                    *paint = Some(Some(Colour(colour)));
                    ui.close();
                }
            }
        });
    }
}
