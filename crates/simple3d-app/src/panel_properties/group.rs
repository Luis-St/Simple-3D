//! A group's boolean operation.

use super::*;
use crate::app::App;
use crate::theme::{self, token};
use simple3d_core::scene::{Body, GroupOp, NodeId};

pub(crate) fn group(app: &mut App, ui: &mut egui::Ui, id: NodeId, current: GroupOp) {
    let mut op = current;
    // Wrapped: the four names overflow the default dock, and overflow widened the column and pushed the Z
    // fields of Position, Rotation and Scale out of reach.
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
        // Name the base child of a difference (spec section 7.3).
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
            // Reorder buttons first, pinned right, so a long name cannot push them out of reach (issue 51).
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
