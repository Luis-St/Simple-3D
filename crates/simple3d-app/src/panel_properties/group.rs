//! A group's boolean operation.

use super::*;
use crate::app::App;
use crate::theme::{self, token};
use simple3d_core::scene::{Body, GroupOp, NodeId};

pub(crate) fn group(app: &mut App, ui: &mut egui::Ui, id: NodeId, current: GroupOp) {
    let mut op = current;
    // Wrapped, not merely laid out left to right: the four names together are
    // wider than the dock at its default width, and a row that overflows widens
    // the whole column behind it -- which is what used to carry the Z field of
    // Position, Rotation and Scale off the panel with no way to reach it.
    field_row(ui, "Operation", "", |ui| {
        for option in GroupOp::ALL {
            if theme::choice(ui, op == option, option.label()).clicked() && op != option {
                op = option;
                app.edit("Operation", None);
                if let Some(node) = app.scene.get_mut(id) {
                    node.body = Body::Group { op };
                }
            }
        }
    });

    let children = app.scene.node(id).children.clone();
    if op.order_matters() {
        // When a difference group is selected, state plainly which child is the
        // base (spec section 7.3).
        match app.scene.difference_base(id) {
            Some(base) => {
                ui.add(
                    egui::Label::new(theme::hint(format!(
                        "Base: {}. Every visible child below it is cut out of it.",
                        app.scene.node(base).name
                    )))
                    .selectable(false),
                );
            }
            None => {
                ui.colored_label(token::ACCENT, "No visible child, so there is nothing to cut.");
            }
        }
    }

    for (index, child) in children.iter().enumerate() {
        let child = *child;
        ui.horizontal(|ui| {
            let name = app.scene.node(child).name.clone();
            let is_base = op.order_matters() && Some(child) == app.scene.difference_base(id);
            let cut = op.order_matters() && !is_base;
            let mark = if is_base {
                "base"
            } else if cut {
                "cut"
            } else {
                ""
            };
            // The two reorder buttons are placed first, pinned to the right-hand
            // edge, and the name takes whatever is left: laid out the other way
            // round, a long name in a narrow panel pushed the buttons off the
            // edge, out of reach (issue 51).
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add_enabled(index + 1 < children.len(), egui::Button::new("\u{25BE}").small()).clicked() {
                    app.edit("Reorder", None);
                    app.scene.reorder(child, 1);
                }
                if ui.add_enabled(index > 0, egui::Button::new("\u{25B4}").small()).clicked() {
                    app.edit("Reorder", None);
                    app.scene.reorder(child, -1);
                }
                if !mark.is_empty() {
                    ui.add(
                        egui::Label::new(egui::RichText::new(mark).size(theme::font::SMALL).color(if cut {
                            token::DANGER
                        } else {
                            token::TEXT_LO
                        }))
                        .selectable(false),
                    );
                }
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    let text = theme::value(format!("{}. {name}", index + 1));
                    if ui.selectable_label(app.is_selected(child), text).clicked() {
                        app.select_only(child);
                    }
                });
            });
        });
    }
    if children.is_empty() {
        ui.add(egui::Label::new(theme::hint("This group is empty.")).selectable(false));
    }
}
