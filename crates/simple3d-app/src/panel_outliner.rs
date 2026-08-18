//! The left dock: the outliner (spec section 7.2) over the primitive palette.
//!
//! The outliner is the centre of gravity of the application, so it gets the
//! dock's height and the palette gets a fixed strip under it. One row per node,
//! 22 px, and the marks that matter -- visibility, the boolean operator, a
//! failure -- are drawn on every row rather than revealed on hover, because a
//! mark you have to go looking for cannot be scanned.

use crate::app::{App, DropTarget, Status};
use crate::icon::{self, Glyph};
use crate::theme::{self, metric, token};
use simple3d_core::scene::{GroupOp, NodeId, Scene};

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

/// The operator mark a node carries in the tree: its own, for a group, or the
/// one it is subject to, for a child of a difference or intersection. Reading
/// the boolean tree at a glance is the whole reason these are inline rather
/// than in a modifier stack somewhere else.
///
/// The flag says the mark is a *cut*: an operand being removed, drawn in the
/// danger colour, which is the one distinction worth a second colour here.
pub fn operator_badge(scene: &Scene, id: NodeId) -> Option<(Glyph, bool)> {
    // The root is a union of everything by definition; badging it says nothing
    // and puts a mark on the one row that never changes.
    if scene.node(id).parent.is_none() {
        return None;
    }
    if let Some(op) = scene.node(id).group_op() {
        return Some((symbol(op), false));
    }
    let parent = scene.node(id).parent?;
    let op = scene.node(parent).group_op()?;
    match op {
        // The base of a difference is what is being cut, not a cut.
        GroupOp::Difference if scene.difference_base(parent) != Some(id) => Some((symbol(op), true)),
        GroupOp::Intersection => Some((symbol(op), false)),
        _ => None,
    }
}

/// The set-theory symbols would be the obvious mark, but the bundled UI face
/// has no glyph for any of them and would draw three identical tofu boxes.
/// These are the same three shapes the tool rail's boolean buttons carry, which
/// makes the tree and the rail read as one vocabulary.
fn symbol(op: GroupOp) -> Glyph {
    match op {
        GroupOp::Union => Glyph::Union,
        GroupOp::Difference => Glyph::Difference,
        GroupOp::Intersection => Glyph::Intersection,
        GroupOp::Hull => Glyph::Polyhedron,
    }
}

/// The outliner's contents, without a dock around them: the dock owns the
/// header bar and decides which side of the window this is on.
pub fn show_inside(app: &mut App, ui: &mut egui::Ui) {
    ui.spacing_mut().item_spacing = egui::vec2(theme::metric::GAP, 2.0);
    if !app.selection.is_empty() {
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.add_space(theme::metric::PANEL_PAD);
            ui.add(egui::Label::new(theme::hint(format!("{} selected", app.selection.len()))).selectable(false));
        });
    }

    confirm_strip(app, ui);

    let dragging = app.outliner_drag;
    app.drop_target = None;
    let ctx = ui.ctx().clone();
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.add_space(2.0);
        let ids = app.scene.depth_first();
        for id in ids {
            row(app, ui, id, dragging);
        }
        // Dropping in the empty space below the tree means "at the end of the
        // root", which is otherwise awkward to reach.
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), ui.available_height().max(24.0)),
            egui::Sense::hover(),
        );
        if dragging.is_some() && response.hovered() {
            let root = app.scene.root();
            app.drop_target = Some(DropTarget { parent: root, index: app.scene.node(root).children.len(), into: None });
            ui.painter().hline(rect.x_range(), rect.top() + 1.0, egui::Stroke::new(2.0_f32, token::ACCENT));
        }
    });

    // Finish the drag on release, wherever the pointer ended up.
    if dragging.is_some() && ctx.input(|i| i.pointer.any_released()) {
        finish_drag(app);
    }
}

/// The inline confirmation for deleting a group.
///
/// It is a strip in the outliner rather than a dialog over the window because
/// the question is about the tree, and the answer is easier to give while
/// still looking at it. The two answers are named for what they do -- neither
/// of them is "OK".
fn confirm_strip(app: &mut App, ui: &mut egui::Ui) {
    if app.pending_delete.is_none() {
        return;
    }
    let names: Vec<String> = app
        .pending_delete
        .as_ref()
        .map(|ids| ids.iter().map(|id| app.scene.node(*id).name.clone()).collect())
        .unwrap_or_default();
    let total = app.pending_delete_count();
    let groups = names.len();
    let children = total - groups;

    egui::Frame::NONE
        .fill(token::DANGER.gamma_multiply(0.16))
        .stroke(egui::Stroke::new(1.0_f32, token::DANGER.gamma_multiply(0.7)))
        .inner_margin(egui::Margin { left: 8, right: 8, top: 6, bottom: 6 })
        .show(ui, |ui| {
            let what = if groups == 1 { format!("\u{201C}{}\u{201D}", names[0]) } else { format!("{groups} groups") };
            ui.add(
                egui::Label::new(theme::value(format!(
                    "Delete {what}? It holds {children} node{}.",
                    if children == 1 { "" } else { "s" }
                )))
                .selectable(false)
                .wrap(),
            );
            ui.horizontal_wrapped(|ui| {
                if ui.button("Delete the children too").on_hover_text("The group and everything inside it").clicked() {
                    app.confirm_delete(false);
                }
                if ui
                    .button("Keep the children")
                    .on_hover_text("The children move up into the group's own place; only the group goes")
                    .clicked()
                {
                    app.confirm_delete(true);
                }
                if ui.button("Cancel").clicked() {
                    app.cancel_delete();
                }
            });
        });
    // Escape is the way out of everything else in this application, so it is
    // the way out of this too.
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.cancel_delete();
    }
}

fn row(app: &mut App, ui: &mut egui::Ui, id: NodeId, dragging: Option<NodeId>) {
    let depth = app.scene.depth(id);
    let node = app.scene.node(id);
    let name = node.name.clone();
    let visible = node.visible;
    let is_group = node.is_group();
    let is_root = id == app.scene.root();
    let selected = app.is_selected(id);
    let primary = app.primary() == Some(id);
    let failed = app.evaluated.error_for(id).is_some();
    let badge = operator_badge(&app.scene, id);
    let type_id = node.spec().map(|s| s.type_id).unwrap_or("");

    let full = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(egui::vec2(full, metric::ROW), egui::Sense::click_and_drag());

    // Background: selection is an accent tint with a solid bar at the left edge,
    // so the primary node of a multi-selection is distinguishable from the rest
    // without a second colour.
    let painter = ui.painter().clone();
    if selected {
        painter.rect_filled(rect, 0.0, token::ACCENT.gamma_multiply(if primary { 0.22 } else { 0.13 }));
        painter.rect_filled(
            egui::Rect::from_min_size(rect.left_top(), egui::vec2(2.0, rect.height())),
            0.0,
            if primary { token::ACCENT } else { token::ACCENT.gamma_multiply(0.5) },
        );
    } else if response.hovered() {
        painter.rect_filled(rect, 0.0, token::SURFACE_3);
    }

    // Visibility lives at the right edge, always drawn, dimmed rather than
    // hidden when the node is not visible.
    let eye_rect =
        egui::Rect::from_min_size(egui::pos2(rect.right() - 22.0, rect.top() + 3.0), egui::Vec2::splat(16.0));
    let eye = ui.interact(eye_rect, ui.id().with((id, "eye")), egui::Sense::click());
    if !is_root {
        let colour = if eye.hovered() {
            token::TEXT_HI
        } else if visible {
            token::TEXT_LO
        } else {
            token::TEXT_LO.gamma_multiply(0.45)
        };
        icon::draw(&painter, eye_rect.shrink(1.0), if visible { Glyph::Eye } else { Glyph::EyeOff }, colour);
        if eye.clicked() {
            app.edit("Toggle visibility", None);
            if let Some(node) = app.scene.get_mut(id) {
                node.visible = !visible;
            }
        }
    }

    let mut x = rect.left() + 6.0 + depth as f32 * 12.0;

    // Type glyph: a bracket for a group, a solid mark for a shape.
    let glyph_rect = egui::Rect::from_min_size(egui::pos2(x, rect.top() + 4.0), egui::Vec2::splat(14.0));
    let glyph = if is_group { Glyph::Bracket } else { Glyph::for_primitive(type_id) };
    let glyph_colour = if failed {
        token::DANGER
    } else if !visible {
        token::TEXT_LO.gamma_multiply(0.5)
    } else if selected {
        token::ACCENT
    } else {
        token::TEXT_LO
    };
    icon::draw(&painter, glyph_rect, glyph, glyph_colour);
    x += 18.0;

    // Rename in place owns the rest of the row while it is open.
    if let Some((rename_id, buffer)) = &mut app.rename {
        if *rename_id == id {
            let field = egui::Rect::from_min_max(
                egui::pos2(x, rect.top() + 1.0),
                egui::pos2(eye_rect.left() - 4.0, rect.bottom() - 1.0),
            );
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(field));
            let response =
                child.add(egui::TextEdit::singleline(buffer).desired_width(f32::INFINITY).font(egui::TextStyle::Body));
            response.request_focus();
            if response.lost_focus() || child.input(|i| i.key_pressed(egui::Key::Enter)) {
                let new_name = buffer.trim().to_string();
                app.rename = None;
                if !new_name.is_empty() && new_name != name {
                    app.edit("Rename", None);
                    if let Some(node) = app.scene.get_mut(id) {
                        node.name = new_name;
                    }
                }
            }
            return;
        }
    }

    // The operator badge sits between the name and the eye, right-aligned, so a
    // column of badges lines up down the tree.
    let mut name_right = eye_rect.left() - 6.0;
    if let Some((mark, subtracted)) = badge {
        let colour = if subtracted { token::DANGER } else { token::TEXT_LO.gamma_multiply(0.8) };
        let badge_rect =
            egui::Rect::from_min_size(egui::pos2(name_right - 14.0, rect.top() + 4.0), egui::Vec2::splat(14.0));
        icon::draw(&painter, badge_rect, mark, colour);
        name_right = badge_rect.left() - 6.0;
    }
    if failed {
        let warn = egui::Rect::from_min_size(egui::pos2(name_right - 14.0, rect.top() + 4.0), egui::Vec2::splat(14.0));
        icon::draw(&painter, warn, Glyph::Warning, token::DANGER);
        name_right = warn.left() - 6.0;
    }

    let text_colour = if failed {
        token::DANGER
    } else if !visible {
        token::TEXT_LO.gamma_multiply(0.6)
    } else if selected {
        token::TEXT_HI
    } else {
        token::TEXT_HI.gamma_multiply(0.85)
    };
    let galley = painter.layout(
        name.clone(),
        egui::FontId::proportional(theme::font::VALUE),
        text_colour,
        (name_right - x).max(1.0),
    );
    painter.galley(egui::pos2(x, rect.center().y - galley.size().y / 2.0), galley, text_colour);

    let response = response.on_hover_text(hover_text(
        app,
        id,
        is_group,
        app.scene.node(id).group_op(),
        app.scene.difference_base(id),
    ));

    // The eye owns its own clicks: pressing it must not also select the row.
    if !eye.hovered() {
        if response.clicked() {
            if ui.input(|i| i.modifiers.command || i.modifiers.shift) {
                app.toggle_selected(id);
            } else {
                app.select_only(id);
            }
        }
        if response.double_clicked() && !is_root {
            app.rename = Some((id, name.clone()));
        }
        if response.drag_started() && !is_root {
            app.outliner_drag = Some(id);
        }
    }

    // Drop indicator.
    if let Some(source) = dragging {
        if response.hovered() {
            let pointer = ui.input(|i| i.pointer.hover_pos()).unwrap_or(rect.center());
            let fraction = ((pointer.y - rect.top()) / rect.height().max(1.0)).clamp(0.0, 1.0);
            let root = app.scene.root();
            if let Some(target) = drop_position(&app.scene, id, fraction, root) {
                let legal = target.parent != source
                    && !app.scene.is_ancestor_of(source, target.parent)
                    && app.scene.node(target.parent).is_group();
                if legal {
                    app.drop_target = Some(target);
                    let stroke = egui::Stroke::new(2.0_f32, token::ACCENT);
                    match target.into {
                        // Into a group: outline the whole row.
                        Some(_) => {
                            painter.rect_stroke(rect, 2.0, stroke, egui::StrokeKind::Inside);
                        }
                        // Between siblings: a line where it will land.
                        None => {
                            let y = if fraction > 0.5 { rect.bottom() } else { rect.top() };
                            painter.hline(rect.x_range(), y, stroke);
                        }
                    }
                } else {
                    // Cycles are prevented, and the indicator says so rather than
                    // letting the user find out on release.
                    painter.rect_stroke(rect, 2.0, egui::Stroke::new(1.0_f32, token::DANGER), egui::StrokeKind::Inside);
                }
            }
        }
    }
}

fn hover_text(
    app: &App,
    id: NodeId,
    is_group: bool,
    op: Option<simple3d_core::scene::GroupOp>,
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
                    Some(base) => {
                        lines.push(format!("Base: {} (everything below it is subtracted)", app.scene.node(base).name))
                    }
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
    use simple3d_core::scene::GroupOp;

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

    #[test]
    fn a_group_wears_its_own_operator_and_a_cut_child_wears_the_cut() {
        let mut scene = Scene::new();
        let root = scene.root();
        let group = scene.add_group(GroupOp::Difference, root, 0);
        let base = scene.add_primitive("box", group, 0).unwrap();
        let cut = scene.add_primitive("cylinder", group, 1).unwrap();

        assert_eq!(operator_badge(&scene, group), Some((Glyph::Difference, false)));
        // The base is what is being cut, so it carries no mark ...
        assert_eq!(operator_badge(&scene, base), None);
        // ... and the operand that does the cutting is marked, in the danger
        // colour, which is what `true` here selects.
        assert_eq!(operator_badge(&scene, cut), Some((Glyph::Difference, true)));
    }

    #[test]
    fn the_root_row_carries_no_operator_mark() {
        let (scene, root, _, _, _) = tree();
        assert_eq!(operator_badge(&scene, root), None);
    }

    #[test]
    fn a_child_of_a_union_carries_no_mark_of_its_own() {
        // Union is the default and the common case; badging every child of one
        // would be noise down the whole tree.
        let (scene, _, _, inner, _) = tree();
        assert_eq!(operator_badge(&scene, inner), None);
    }
}
