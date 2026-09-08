//! Choosing which nodes become objects of their own.

use super::*;
use crate::app::{App, Modal};
use crate::theme;
use crate::ui;
use simple3d_core::scene::{ExportBody, NodeId, Scene};

impl App {
    /// What the export is about to do. Drawn before the rest of the window and
    /// at the bottom of it, so a body list long enough to scroll cannot push the
    /// count out of sight.
    pub(super) fn export_summary_line(&mut self, ui: &mut egui::Ui) {
        if self.evaluated.errors.is_empty() {
            // The count is of what will actually be written -- the scene
            // or the selection -- rather than always of the whole scene.
            let summary = self.export_summary();
            let bodies = match summary.bodies {
                1 => "one body".to_string(),
                n => format!("{n} bodies"),
            };
            ui.label(format!(
                "{} triangles in {bodies}, each verified as watertight before anything is written.",
                summary.triangles
            ));
        } else {
            ui.colored_label(
                ui.visuals().error_fg_color,
                "The scene has geometry that could not be evaluated; export will refuse.",
            );
        }
        ui.separator();
    }

    /// The rows the body picker shows: the export's own roots, and the children
    /// of every group that has been split open, in tree order.
    ///
    /// Only what can be answered is listed. A group that is a body is one solid
    /// and what is inside it is not a decision to be made -- splitting it is
    /// what turns its children into rows, which is also what makes the list
    /// short enough to read on a scene of any size.
    pub(super) fn body_rows(&self) -> Vec<(NodeId, usize)> {
        fn walk(scene: &Scene, id: NodeId, depth: usize, rows: &mut Vec<(NodeId, usize)>) {
            if !scene.contains(id) {
                return;
            }
            rows.push((id, depth));
            if scene.node(id).export_body == Some(ExportBody::Split) && scene.can_split_for_export(id) {
                for &child in &scene.node(id).children {
                    walk(scene, child, depth + 1, rows);
                }
            }
        }
        let mut rows = Vec::new();
        for id in self.export_roots() {
            walk(&self.scene, id, 0, &mut rows);
        }
        rows
    }

    /// Which body each node goes in, chosen a row at a time. The marks live on
    /// the nodes, so they are saved with the project and a later export only
    /// has to say what has changed since (issue 58).
    pub(super) fn body_picker(&mut self, ui: &mut egui::Ui) {
        let rows = self.body_rows();
        if rows.is_empty() {
            ui.label(theme::hint("Nothing to export, so there are no bodies to group."));
            return;
        }
        // One past the highest in use, so picking the last entry is how a new
        // body gets made.
        let offered = self.scene.highest_export_body() + 1;
        let mut change: Option<(NodeId, Option<ExportBody>)> = None;

        egui::Frame::NONE
            .fill(theme::token::SURFACE_2)
            .stroke(egui::Stroke::new(1.0_f32, theme::token::SURFACE_3))
            .inner_margin(egui::Margin::same(6))
            .show(ui, |ui| {
                // As tall as the list needs, up to whatever the window has
                // left under the controls above it; past that it scrolls.
                let room = (ui.available_height() - 24.0).max(80.0);
                let (area, restore) = theme::list_scroll_area(ui);
                area.max_height(room).auto_shrink([false, true]).show(ui, |ui| {
                    ui.set_style(restore);
                    for (id, depth) in rows {
                        let node = self.scene.node(id);
                        let (name, visible, mark) = (node.name.clone(), node.visible, node.export_body);
                        let splittable = self.scene.can_split_for_export(id);
                        ui.horizontal(|ui| {
                            ui.add_space(depth as f32 * 14.0);
                            let label = if visible {
                                theme::value(&name)
                            } else {
                                theme::hint(format!("{name} (hidden, not exported)"))
                            };
                            ui.add(egui::Label::new(label).selectable(false).truncate());
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let mut chosen = mark;
                                egui::ComboBox::from_id_salt(("export-body", id))
                                    .width(150.0)
                                    .selected_text(body_label(mark))
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut chosen, None, body_label(None));
                                        for key in 1..=offered {
                                            let shared = Some(ExportBody::Shared(key));
                                            ui.selectable_value(&mut chosen, shared, body_label(shared));
                                        }
                                        if splittable {
                                            let split = Some(ExportBody::Split);
                                            ui.selectable_value(&mut chosen, split, body_label(split));
                                        }
                                    });
                                if chosen != mark {
                                    change = Some((id, chosen));
                                }
                            });
                        });
                    }
                });
            });
        ui.label(theme::hint(
            "A body of its own is one component. The same number on two shapes writes them as one solid. \
             Splitting a group offers what is inside it.",
        ));

        if let Some((id, body)) = change {
            self.edit("Group export bodies", None);
            self.scene.set_export_body(id, body);
        }
    }

    pub(super) fn close_action(&mut self, ui: &mut egui::Ui) {
        if ui::dialog_button(ui, "Close", true).clicked() {
            self.modal = Modal::None;
        }
    }
}
