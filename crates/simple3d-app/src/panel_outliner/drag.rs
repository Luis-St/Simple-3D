//! Dragging rows about the tree.

use super::*;
use crate::app::{App, Carried, DropTarget, Status};
use crate::icon::{self, Glyph};
use crate::theme::{self, metric, token};
use simple3d_core::scene::{NodeId, Scene};

/// Where a pointer at `fraction` down a row would drop: into the row's node, or
/// between it and one of its siblings. Kept separate from the drawing so the rule
/// can be reasoned about (and tested) on its own.
///
/// `open` says the row is a group showing its children, which changes what the
/// gap under it means (issue 49).
pub fn drop_position(scene: &Scene, over: NodeId, fraction: f32, root: NodeId, open: bool) -> Option<DropTarget> {
    let node = scene.get(over)?;
    let is_group = node.can_hold_children();
    // The top and bottom fifths of a row mean "beside"; the middle means "into",
    // but only for a group or pattern, since nothing else can hold children.
    let before = fraction < 0.25;
    let after = fraction > 0.75;
    if is_group && !before && !after {
        return Some(DropTarget { parent: over, index: scene.node(over).children.len(), into: Some(over) });
    }
    // The gap under an open group's row is the gap above its first child, and
    // what lands in it is the group's new first child. Read as "beside the
    // group" it landed in the group's parent instead -- a level out from the
    // one the line was drawn at, which is not where the pointer was aiming
    // (issue 49). A shut group has nothing under it, so its gap still means
    // "beside".
    if is_group && after && open {
        return Some(DropTarget { parent: over, index: 0, into: None });
    }
    if over == root {
        // The root has no siblings, so any drop on it goes inside.
        return Some(DropTarget { parent: root, index: scene.node(root).children.len(), into: Some(root) });
    }
    let parent = node.parent?;
    let index = scene.node(parent).children.iter().position(|&c| c == over)?;
    Some(DropTarget { parent, index: if after { index + 1 } else { index }, into: None })
}

/// Where the line marking a drop *between* two rows is drawn: the middle of the
/// gap between them, whichever of the two the pointer happens to be over.
///
/// One gap is one landing place, so it gets one line. Drawn on the row's own
/// edge instead, the same drop showed as two lines a spacing apart -- one under
/// the row above, one over the row below -- and reads as two positions to choose
/// between (issue 48).
pub fn gap_line_y(rect: egui::Rect, spacing: f32, after: bool) -> f32 {
    let half = spacing / 2.0;
    if after {
        rect.bottom() + half
    } else {
        rect.top() - half
    }
}

/// What is being dragged, drawn under the pointer: the node's own glyph and
/// name on a translucent slab.
///
/// The row itself stays in the tree as a shadow of what it was (issue 46), so
/// the two together say the whole thing: the shadow is where the load came
/// from, the slab is what is held and how much of it, and the drop line is
/// where it would land.
pub(crate) fn drag_ghost(app: &App, ctx: &egui::Context, load: Carried, carried: usize) {
    let Some(pointer) = ctx.input(|i| i.pointer.hover_pos()) else { return };
    let (glyph, name) = match load {
        Carried::Rows(source) => {
            if !app.scene.contains(source) {
                return;
            }
            let node = app.scene.node(source);
            let glyph = node_glyph(node);
            // A whole selection travels under one slab, named for how much of
            // it there is: eight slabs stacked on the pointer would cover the
            // drop indicator they exist to point at.
            let name = if carried > 1 { format!("{carried} nodes") } else { node.name.clone() };
            (glyph, name)
        }
        // A shape out of the palette wears the same slab, so the two drags read
        // as the one gesture: the tile it came from is the picture on it, and
        // the shape's own name is the text.
        Carried::Shape(type_id) => (
            Glyph::for_primitive(type_id),
            simple3d_core::primitive::lookup(type_id).map(|spec| spec.label).unwrap_or(type_id).to_string(),
        ),
    };
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("outliner-drag-ghost")));
    let galley = painter.layout_no_wrap(
        name,
        egui::FontId::proportional(theme::font::VALUE),
        token::TEXT_HI.gamma_multiply(0.9),
    );
    // Held a little below and to the right of the pointer, so the drop
    // indicator under the pointer is never covered by what is being dropped.
    let at = pointer + egui::vec2(14.0, 6.0);
    let slab = egui::Rect::from_min_size(at, egui::vec2(galley.size().x + 26.0, metric::ROW)).expand(1.0);
    painter.rect_filled(slab, 3.0, token::SURFACE_2.gamma_multiply(0.72));
    painter.rect_stroke(
        slab,
        3.0,
        egui::Stroke::new(1.0_f32, token::ACCENT.gamma_multiply(0.55)),
        egui::StrokeKind::Inside,
    );
    icon::draw(
        &painter,
        egui::Rect::from_min_size(at + egui::vec2(4.0, 4.0), egui::Vec2::splat(14.0)),
        glyph,
        token::ACCENT.gamma_multiply(0.75),
    );
    painter.galley(
        egui::pos2(at.x + 22.0, slab.center().y - galley.size().y / 2.0),
        galley,
        token::TEXT_HI.gamma_multiply(0.9),
    );
}

/// Whether everything being dragged may land on `target`: nothing can be
/// dropped into itself or into anything it holds, and only a group or a pattern
/// can hold children at all.
pub fn drop_is_legal(scene: &Scene, carried: &[NodeId], target: &DropTarget) -> bool {
    scene.node(target.parent).can_hold_children()
        && carried.iter().all(|&source| target.parent != source && !scene.is_ancestor_of(source, target.parent))
}

pub(crate) fn finish_drag(app: &mut App) {
    let Some(load) = app.outliner_drag.take() else { return };
    let Some(target) = app.drop_target.take() else { return };
    // A shape from the palette is added where the indicator said, rather than
    // moved: there is nothing in the tree yet to move.
    let source = match load {
        Carried::Rows(source) => source,
        Carried::Shape(type_id) => {
            app.add_dropped_primitive(type_id, target.parent, target.index);
            return;
        }
    };
    let carried: Vec<NodeId> = app.dragged_nodes(source).into_iter().filter(|id| app.scene.contains(*id)).collect();
    if carried.is_empty() {
        return;
    }
    app.edit("Reparent", None);
    match app.scene.reparent_many(&carried, target.parent, target.index) {
        Ok(()) => {
            // A pattern that has just gained its first child can now be measured,
            // so its stock spacing is replaced by one the shapes stand clear at
            // (issue 67).
            app.size_fresh_patterns();
            // The load stays selected, in the order it landed in: a drag that
            // dropped the rest of the selection on arrival would make moving
            // several nodes twice in a row impossible.
            app.select_only(carried[0]);
            for &id in &carried[1..] {
                app.toggle_selected(id);
            }
            let into = app.scene.node(target.parent).name.clone();
            app.status = Status::Info(if carried.len() > 1 {
                format!("Moved {} nodes into {into}", carried.len())
            } else {
                format!("Moved into {into}")
            });
        }
        Err(why) => {
            app.history.discard_last();
            app.status = Status::Warning(why.to_string());
        }
    }
}
