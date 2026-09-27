//! Dragging rows about the tree.

use super::*;
use crate::app::{App, Carried, DropTarget, Status};
use crate::icon::{self, Glyph};
use crate::theme::{self, metric, token};
use simple3d_core::scene::{NodeId, Scene};

/// Where a pointer `fraction` down a row would drop: into the row's node or beside it. `open`
/// marks a group showing its children, which changes the gap under it (issue 49).
pub fn drop_position(scene: &Scene, over: NodeId, fraction: f32, root: NodeId, open: bool) -> Option<DropTarget> {
    let node = scene.get(over)?;
    let is_group = node.can_hold_children();
    // The top and bottom quarters mean "beside"; the middle means "into", for groups and patterns only.
    let before = fraction < 0.25;
    let after = fraction > 0.75;
    if is_group && !before && !after {
        return Some(DropTarget { parent: over, index: scene.node(over).children.len(), into: Some(over) });
    }
    // The gap under an open group lands as its first child, matching where the line is drawn
    // (issue 49); a shut group's gap still means "beside".
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

/// Where a between-rows drop line is drawn: mid-gap, whichever row the pointer is over, so one
/// gap shows one line (issue 48).
pub fn gap_line_y(rect: egui::Rect, spacing: f32, after: bool) -> f32 {
    let half = spacing / 2.0;
    if after {
        rect.bottom() + half
    } else {
        rect.top() - half
    }
}

/// The dragged load under the pointer: glyph and name on a translucent slab. The row stays as a
/// shadow (issue 46).
pub(crate) fn drag_ghost(app: &App, ctx: &egui::Context, load: Carried, carried: usize) {
    let Some(pointer) = ctx.input(|i| i.pointer.hover_pos()) else { return };
    let (glyph, name) = match load {
        Carried::Rows(source) => {
            if !app.scene.contains(source) {
                return;
            }
            let node = app.scene.node(source);
            let glyph = node_glyph(node);
            // A multi-selection travels under one slab, named by its count.
            let name = if carried > 1 { format!("{carried} nodes") } else { node.name.clone() };
            (glyph, name)
        }
        // A palette shape wears the same slab, so both drags read as one gesture.
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
    // Offset from the pointer so the drop indicator stays visible.
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

/// Whether everything dragged may land on `target`: not into itself or its descendants, and only
/// into groups or patterns.
pub fn drop_is_legal(scene: &Scene, carried: &[NodeId], target: &DropTarget) -> bool {
    scene.node(target.parent).can_hold_children()
        && carried.iter().all(|&source| target.parent != source && !scene.is_ancestor_of(source, target.parent))
}

pub(crate) fn finish_drag(app: &mut App) {
    let Some(load) = app.outliner_drag.take() else { return };
    let Some(target) = app.drop_target.take() else { return };
    // A palette shape is added where the indicator said, not moved.
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
            // A pattern's first child lets it be sized so shapes stand clear (issue 67).
            app.size_fresh_patterns();
            // The load stays selected, so it can be moved again at once.
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
