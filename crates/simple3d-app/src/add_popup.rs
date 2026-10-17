//! Setting a shape up before it is added: from a palette tile's menu, an in-place popup with the
//! shape's parameters, where it lands and its position. The shape is in the scene while the popup is
//! open, so the viewport previews every change; Add keeps it as one undo step, Cancel takes it out.
//! Saved shapes are not offered it, having no parameters of their own.

use crate::app::{App, Status};
use crate::popup::{PopupEvent, PopupSpec};
use crate::theme;
use crate::ui;
use simple3d_core::config::Placement;
use simple3d_core::keymap::{Action, Command};
use simple3d_core::scene::{GroupOp, NodeId};

/// The popup's key, which also remembers where it was dragged.
const KEY: &str = "add-primitive";

/// The window width: a parameter's name and field side by side, and the three position fields.
const WIDTH: f32 = 320.0;

/// The shape being set up.
#[derive(Clone, Debug)]
pub(crate) struct Adding {
    pub(crate) id: NodeId,
    type_id: &'static str,
    label: &'static str,
    /// Where it lands, for this shape only; the document's setting is left alone.
    placement: Placement,
    /// The undo stack before it was added, so Add and Cancel know which steps are the popup's.
    mark: u64,
}

impl App {
    /// Add `type_id` from its tile, the Add menu or its shortcut: straight in, or through the popup while
    /// the add-with-settings key is held.
    pub(crate) fn add_primitive(&mut self, type_id: &'static str, settings: bool) {
        if settings {
            self.open_add_popup(type_id);
        } else {
            self.add_node(Some(type_id), GroupOp::Union);
        }
    }

    /// Whether the add-with-settings key is held now.
    pub(crate) fn settings_held(&self, ctx: &egui::Context) -> bool {
        ctx.input(|i| self.holding(Command::AddWithSettings, |key| i.key_down(key), i.modifiers))
    }

    /// The primitive bound to a press made with the add-with-settings key held, its modifiers taken off
    /// the press, since Shift+B is not B's binding.
    pub(crate) fn primitive_under_settings_key(
        &self,
        pressed: &str,
        down: impl Fn(&str) -> bool,
        mods: egui::Modifiers,
    ) -> Option<&'static str> {
        let hold = self.keymap.binding(Command::AddWithSettings)?;
        let (ctrl, shift, alt) = (mods.command && !hold.ctrl, mods.shift && !hold.shift, mods.alt && !hold.alt);
        // The hold key's own keys are not part of the shape's binding either.
        let down = |name: &str| down(name) && !hold.keys.iter().any(|key| key == name);
        match self.keymap.action_for_press(pressed, down, ctrl, shift, alt)? {
            Action::Primitive(type_id) => simple3d_core::primitive::lookup(&type_id).map(|spec| spec.type_id),
            Action::Command(_) => None,
        }
    }

    /// Add `type_id` and open the popup on it. A shape still being set up or put down is kept first.
    pub(crate) fn open_add_popup(&mut self, type_id: &'static str) {
        let Some(spec) = simple3d_core::primitive::lookup(type_id) else { return };
        self.confirm_add_popup();
        if self.placing.is_some() {
            self.finish_placing();
        }
        let mark = self.history.mark();
        // "On a surface" puts it at the cursor until Add lets it follow the pointer.
        let placement = self.settings.placement;
        let Some(id) = self.insert_new(Some(type_id), GroupOp::Union, placement) else { return };
        self.add_popup = Some(Adding { id, type_id, label: spec.label, placement, mark });
        self.status = Status::Info(format!("Set the {} up, then Add it", spec.label.to_lowercase()));
    }

    /// Keep the shape, its setting up one step with its addition; under "On a surface" it then
    /// follows the pointer as any new shape does.
    pub(crate) fn confirm_add_popup(&mut self) {
        let Some(adding) = self.add_popup.take() else { return };
        if !self.scene.contains(adding.id) {
            return;
        }
        self.history.squash_since(adding.mark);
        self.status = Status::Info(format!("Added {}", self.scene.node(adding.id).name));
        if adding.placement == Placement::OnSurface {
            self.start_placing_now(adding.id);
        }
    }

    /// Take the shape out again, with everything set on it.
    pub(crate) fn cancel_add_popup(&mut self) {
        let Some(adding) = self.add_popup.take() else { return };
        self.history.squash_since(adding.mark);
        if self.scene.contains(adding.id) && self.history.mark() > adding.mark {
            self.undo_now();
            self.status = Status::Info(format!("Did not add the {}", adding.label.to_lowercase()));
        }
    }

    /// Move the shape to where `placement` puts it.
    pub(crate) fn land_added(&mut self, placement: Placement) {
        let Some(adding) = self.add_popup.as_mut() else { return };
        adding.placement = placement;
        let id = adding.id;
        let Some(parent) = self.scene.get(id).and_then(|node| node.parent) else { return };
        let near = self.near_face(placement, &[id]).unwrap_or(0.0);
        let at = self.landing_in(placement, parent, near);
        self.edit("Add at", None);
        if let Some(node) = self.scene.get_mut(id) {
            node.position = at;
        }
    }
}

/// The window over the viewport, while a shape is being set up.
pub(crate) fn show(app: &mut App, ctx: &egui::Context) {
    // Not modal, so the shape may be deleted or undone away while it is open.
    if app.add_popup.as_ref().is_some_and(|adding| !app.scene.contains(adding.id)) {
        app.add_popup = None;
    }
    let Some(adding) = app.add_popup.clone() else { return };
    let title = format!("Add a {}", adding.label.to_lowercase());
    let spec = PopupSpec { key: KEY, title: &title, width: WIDTH };
    let event = app.tool_popup(ctx, spec, |app, ui| body(app, ui, &adding), actions);
    if event == PopupEvent::Closed {
        app.cancel_add_popup();
    }
}

fn body(app: &mut App, ui: &mut egui::Ui, adding: &Adding) {
    let targets = [adding.id];
    crate::panel_properties::primitive_as(app, ui, &targets, adding.type_id, crate::panel_properties::ADD_POPUP_ROW);
    ui.add_space(4.0);
    crate::panel_properties::field_row(
        ui,
        "Add at",
        "Where this shape lands; the document's choice stays as it is",
        |ui| {
            let mut chosen = adding.placement;
            egui::ComboBox::from_id_salt("add-popup-placement").selected_text(theme::value(chosen.label())).show_ui(
                ui,
                |ui| {
                    for option in Placement::ALL {
                        ui.selectable_value(&mut chosen, option, option.label());
                    }
                },
            );
            if chosen != adding.placement {
                app.land_added(chosen);
            }
        },
    );
    if adding.placement == Placement::OnSurface {
        ui.label(theme::hint("After Add it follows the pointer onto a surface; click to put it down."));
    }
    crate::panel_properties::transform(app, ui, &targets, crate::panel_properties::ADD_POPUP_ROW);
}

fn actions(app: &mut App, ui: &mut egui::Ui) {
    // The action row lays out from the right, so buttons are added in reverse reading order.
    if ui::dialog_button(ui, "Add", true).clicked() {
        app.confirm_add_popup();
    }
    if ui::dialog_button(ui, "Cancel", true).clicked() {
        app.cancel_add_popup();
    }
}
