//! An integration of a component (issue 113).

use super::*;
use crate::app::App;
use crate::theme::{self, token};
use simple3d_core::scene::{Body, GroupOp, NodeId};

/// An integration's panel: its component, a way in, and the combining operation, the one inner choice
/// it owns; everything else is edited in the component's tab.
pub(crate) fn component_body(app: &mut App, ui: &mut egui::Ui, id: NodeId, open: &mut bool) {
    let Body::Component { component, op } = app.scene.node(id).body else { return };
    let Some(name) = app.component_label(component) else {
        ui.colored_label(token::DANGER, "The component it places is no longer in the project.");
        return;
    };
    field_row(ui, "Component", "", |ui| {
        ui.add(egui::Label::new(theme::value(&name)).selectable(false));
    });
    let own = app.component_scene(component).and_then(|scene| scene.node(scene.root()).group_op());
    let mut chosen = op;
    field_row(ui, "Operation", "", |ui| {
        let label = match own {
            Some(own) => format!("As the component ({})", own.label().to_lowercase()),
            None => "As the component".to_string(),
        };
        if theme::choice(ui, chosen.is_none(), &label).clicked() {
            chosen = None;
        }
        for option in GroupOp::ALL {
            if theme::choice(ui, chosen == Some(option), option.label()).clicked() {
                chosen = Some(option);
            }
        }
    });
    if chosen != op {
        app.edit("Operation", None);
        if let Some(node) = app.scene.get_mut(id) {
            node.body = Body::Component { component, op: chosen };
        }
    }
    let uses = app.integration_count(component);
    let others = match uses {
        0 | 1 => "It is placed nowhere else.".to_string(),
        2 => "It is placed in one other place too, which shows the same edits.".to_string(),
        n => format!("It is placed in {} other places too, which show the same edits.", n - 1),
    };
    ui.add(
        egui::Label::new(theme::hint(format!(
            "What is inside is edited in the component's own tab. {others} Where it stands, whether it is shown \
             and its operation are this placement's own."
        )))
        .selectable(false),
    );
    if ui.button(format!("Open {name}")).on_hover_text("Edit the component in its own tab").clicked() {
        *open = true;
    }
}
