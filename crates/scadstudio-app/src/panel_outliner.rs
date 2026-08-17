//! The outliner (spec section 7.2): the scene tree, with rename in place,
//! visibility toggles, multi-selection, reordering and reparenting by dragging.

use crate::app::{App, DropTarget, Status};
use scadstudio_core::scene::{NodeId, Scene};

/// Where a pointer at `fraction` down a row would drop: into the row's node, or
/// between it and one of its siblings. Kept separate from the drawing so the rule
/// can be reasoned about (and tested) on its own.
pub fn drop_position(scene: &Scene, over: NodeId, fraction: f32, root: NodeId) -> Option<DropTarget> {
    let node = scene.get(over)?;
    let is_group = node.is_group();
    // The top and bottom fifths of a row mean "beside"; the middle means "into",
    // but only for a group, since nothing else can hold children.
    let before = fraction < 0.25;
    let after = fraction > 0.75;
    if is_group && !before && !after {
        return Some(DropTarget { parent: over, index: scene.node(over).children.len(), into: Some(over) });
    }
    if over == root {
        // The root has no siblings, so any drop on it goes inside.
        return Some(DropTarget { parent: root, index: scene.node(root).children.len(), into: Some(root) });
    }
    let parent = node.parent?;
    let index = scene.node(parent).children.iter().position(|&c| c == over)?;
    Some(DropTarget { parent, index: if after { index + 1 } else { index }, into: None })
}

pub fn show(app: &mut App, ctx: &egui::Context) {
    let mut width = app.settings.outliner_width;
    egui::SidePanel::left("outliner")
        .resizable(true)
        .default_width(width)
        .width_range(180.0..=520.0)
        .show(ctx, |ui| {
            width = ui.available_width();
            ui.horizontal(|ui| {
                ui.heading("Outliner");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("Delete").clicked() {
                        app.run(scadstudio_core::keymap::Command::Delete);
                    }
                    if ui.small_button("Group").clicked() {
                        app.run(scadstudio_core::keymap::Command::Group);
                    }
                });
            });
            ui.separator();

            let dragging = app.outliner_drag;
            app.drop_target = None;
            egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                let ids = app.scene.depth_first();
                for id in ids {
                    row(app, ui, id, dragging);
                }
                // Dropping in the empty space below the tree means "at the end of
                // the root", which is otherwise awkward to reach.
                let (rect, response) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), ui.available_height().max(24.0)),
                    egui::Sense::hover(),
                );
                if dragging.is_some() && response.hovered() {
                    let root = app.scene.root();
                    app.drop_target =
                        Some(DropTarget { parent: root, index: app.scene.node(root).children.len(), into: None });
                    ui.painter().hline(
                        rect.x_range(),
                        rect.top() + 1.0,
                        egui::Stroke::new(2.0_f32, ui.visuals().selection.bg_fill),
                    );
                }
            });

            // Finish the drag on release, wherever the pointer ended up.
            if dragging.is_some() && ctx.input(|i| i.pointer.any_released()) {
                finish_drag(app);
            }
        });
    app.settings.outliner_width = width;
}

fn row(app: &mut App, ui: &mut egui::Ui, id: NodeId, dragging: Option<NodeId>) {
    let depth = app.scene.depth(id);
    let node = app.scene.node(id);
    let name = node.name.clone();
    let visible = node.visible;
    let is_group = node.is_group();
    let group_op = node.group_op();
    let is_root = id == app.scene.root();
    let selected = app.is_selected(id);
    let failed = app.evaluated.error_for(id).is_some();
    let base_child = group_op
        .filter(|op| op.order_matters())
        .and_then(|_| app.scene.difference_base(id));

    let row_response: Option<egui::Response> = ui
        .horizontal(|ui| {
            ui.add_space(depth as f32 * 14.0);

            // Visibility toggle, directly in the tree.
            let eye = if visible { "O" } else { "-" };
            if ui
                .add_enabled(!is_root, egui::Button::new(eye).small())
                .on_hover_text(if visible { "Visible" } else { "Hidden" })
                .clicked()
            {
                app.edit("Toggle visibility", None);
                if let Some(node) = app.scene.get_mut(id) {
                    node.visible = !visible;
                }
            }

            // Rename in place.
            if let Some((rename_id, buffer)) = &mut app.rename {
                if *rename_id == id {
                    let response = ui.add(egui::TextEdit::singleline(buffer).desired_width(160.0));
                    response.request_focus();
                    let commit = response.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if commit {
                        let new_name = buffer.trim().to_string();
                        app.rename = None;
                        if !new_name.is_empty() && new_name != name {
                            app.edit("Rename", None);
                            if let Some(node) = app.scene.get_mut(id) {
                                node.name = new_name;
                            }
                        }
                    }
                    return None;
                }
            }

            let mut text = egui::RichText::new(&name);
            if !visible {
                text = text.weak();
            }
            if failed {
                // A boolean that could not be evaluated names its node here
                // (spec section 5.2).
                text = text.color(ui.visuals().error_fg_color);
            }
            let label = ui
                .selectable_label(selected, text)
                .on_hover_text(hover_text(app, id, is_group, group_op, base_child));

            if is_group {
                if let Some(op) = group_op {
                    ui.label(egui::RichText::new(op.label()).weak().small());
                }
            }
            Some(label)
        })
        .inner;

    // `None` while this row is being renamed: the text field owns the row, and
    // clicking or dragging it must not also select or reparent the node.
    let Some(row_response) = row_response else { return };
    let interact = ui.interact(row_response.rect, row_response.id.with("drag"), egui::Sense::click_and_drag());

    if interact.clicked() {
        if ui.input(|i| i.modifiers.command || i.modifiers.shift) {
            app.toggle_selected(id);
        } else {
            app.select_only(id);
        }
    }
    if interact.double_clicked() && !is_root {
        app.rename = Some((id, name.clone()));
    }
    if interact.drag_started() && !is_root {
        app.outliner_drag = Some(id);
    }

    // Drop indicator.
    if let Some(source) = dragging {
        if interact.hovered() || row_response.hovered() {
            let rect = row_response.rect;
            let pointer = ui.input(|i| i.pointer.hover_pos()).unwrap_or(rect.center());
            let fraction = ((pointer.y - rect.top()) / rect.height().max(1.0)).clamp(0.0, 1.0);
            let root = app.scene.root();
            if let Some(target) = drop_position(&app.scene, id, fraction, root) {
                let legal = target.parent != source
                    && !app.scene.is_ancestor_of(source, target.parent)
                    && app.scene.node(target.parent).is_group();
                if legal {
                    app.drop_target = Some(target);
                    let stroke = egui::Stroke::new(2.0_f32, ui.visuals().selection.bg_fill);
                    match target.into {
                        // Into a group: outline the whole row.
                        Some(_) => {
                            ui.painter().rect_stroke(rect, 2.0, stroke, egui::StrokeKind::Inside);
                        }
                        // Between siblings: a line where it will land.
                        None => {
                            let y = if fraction > 0.5 { rect.bottom() } else { rect.top() };
                            ui.painter().hline(rect.x_range(), y, stroke);
                        }
                    }
                } else {
                    // Cycles are prevented, and the indicator says so rather than
                    // letting the user find out on release.
                    ui.painter().rect_stroke(
                        rect,
                        2.0,
                        egui::Stroke::new(1.0_f32, ui.visuals().error_fg_color),
                        egui::StrokeKind::Inside,
                    );
                }
            }
        }
    }
}

fn hover_text(
    app: &App,
    id: NodeId,
    is_group: bool,
    op: Option<scadstudio_core::scene::GroupOp>,
    base_child: Option<NodeId>,
) -> String {
    let mut lines: Vec<String> = Vec::new();
    if let Some(error) = app.evaluated.error_for(id) {
        lines.push(error.message.clone());
    }
    if is_group {
        if let Some(op) = op {
            lines.push(format!("{} of {} children", op.label(), app.scene.node(id).children.len()));
            if op.order_matters() {
                match base_child {
                    Some(base) => lines
                        .push(format!("Base: {} (everything below it is subtracted)", app.scene.node(base).name)),
                    None => lines.push("No visible child to use as the base".into()),
                }
            }
        }
    } else if let Some(spec) = app.scene.node(id).spec() {
        lines.push(spec.label.to_string());
    }
    if let Some(mesh) = app.evaluated.node_meshes.get(&id) {
        if let Some((lo, hi)) = mesh.bounds() {
            lines.push(crate::ui::describe_size(hi - lo, app.unit()));
        }
    }
    lines.join("\n")
}

fn finish_drag(app: &mut App) {
    let Some(source) = app.outliner_drag.take() else { return };
    let Some(target) = app.drop_target.take() else { return };
    if !app.scene.contains(source) {
        return;
    }
    app.edit("Reparent", None);
    match app.scene.reparent(source, target.parent, target.index) {
        Ok(()) => {
            app.select_only(source);
            app.status = Status::Info(format!("Moved into {}", app.scene.node(target.parent).name));
        }
        Err(why) => app.status = Status::Warning(why.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scadstudio_core::scene::GroupOp;

    fn tree() -> (Scene, NodeId, NodeId, NodeId, NodeId) {
        let mut scene = Scene::new();
        let root = scene.root();
        let group = scene.add_group(GroupOp::Union, root, 0);
        let inner = scene.add_primitive("box", group, 0).unwrap();
        let sibling = scene.add_primitive("box", root, 1).unwrap();
        (scene, root, group, inner, sibling)
    }

    #[test]
    fn the_middle_of_a_group_row_drops_into_it() {
        let (scene, root, group, _, _) = tree();
        let target = drop_position(&scene, group, 0.5, root).unwrap();
        assert_eq!(target.into, Some(group));
        assert_eq!(target.parent, group);
    }

    #[test]
    fn the_edges_of_a_row_drop_beside_it() {
        let (scene, root, group, _, sibling) = tree();
        let before = drop_position(&scene, sibling, 0.05, root).unwrap();
        assert_eq!(before.parent, root);
        assert_eq!(before.index, 1);
        assert_eq!(before.into, None);

        let after = drop_position(&scene, sibling, 0.95, root).unwrap();
        assert_eq!(after.index, 2);

        // A group's edges also mean "beside", not "into".
        let beside_group = drop_position(&scene, group, 0.05, root).unwrap();
        assert_eq!(beside_group.parent, root);
        assert_eq!(beside_group.index, 0);
        assert_eq!(beside_group.into, None);
    }

    #[test]
    fn a_leaf_row_never_drops_into_itself() {
        let (scene, root, _, inner, _) = tree();
        for fraction in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let target = drop_position(&scene, inner, fraction, root).unwrap();
            assert_eq!(target.into, None, "a leaf accepted a child at {fraction}");
        }
    }

    #[test]
    fn any_drop_on_the_root_goes_inside_it() {
        let (scene, root, _, _, _) = tree();
        for fraction in [0.0, 0.5, 1.0] {
            let target = drop_position(&scene, root, fraction, root).unwrap();
            assert_eq!(target.parent, root);
            assert_eq!(target.into, Some(root));
        }
    }

    #[test]
    fn the_index_a_drop_reports_is_the_one_reparent_expects() {
        // The two have to agree, or a drop lands one row away from the indicator.
        let (mut scene, root, group, inner, sibling) = tree();
        let target = drop_position(&scene, sibling, 0.95, root).unwrap();
        scene.reparent(inner, target.parent, target.index).unwrap();
        assert_eq!(scene.node(root).children, vec![group, sibling, inner]);
    }
}
